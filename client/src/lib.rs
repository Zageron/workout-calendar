mod utils;

use std::cell::RefCell;
use std::rc::Rc;

use log::debug;
use log::info;
use log::trace;
use utils::to_js_array;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{DataTransfer, Document, DomTokenList, DragEvent, Element, Node, NodeList, Window};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "console_log")] {
        #[allow(dead_code)]
        fn init_log() {
            use log::Level;
            console_log::init_with_level(Level::Trace).expect("error initializing log");
        }
    } else {
        fn init_log() {}
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    init_log();
    let window: Box<Window> = Box::new(web_sys::window().expect("no global `window` exists"));
    let document: Box<Document> =
        Box::new(window.document().expect("should have a document on window"));

    let node_items = Box::new(document.query_selector_all(".draggable").unwrap());

    add_drag_and_drop_listeners(node_items)
}

fn add_drag_and_drop_listeners(_node_list: Box<NodeList>) -> Result<(), JsValue> {
    let drag_active: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let drag_source_content: Rc<RefCell<Option<web_sys::EventTarget>>> =
        Rc::new(RefCell::new(None));
    {
        let node_list = _node_list.clone();
        let captured_source_content = drag_source_content.clone();
        let dragging = drag_active.clone();
        let drag_start = Closure::wrap(Box::new(move |event: DragEvent| {
            if !*dragging.borrow() {
                let target: web_sys::EventTarget = event.target().unwrap();
                let element: &Element = target.dyn_ref::<Element>().unwrap();
                let data_transfer: DataTransfer = event.data_transfer().unwrap();

                let class_list: DomTokenList = element.class_list();
                class_list.add(&to_js_array(&["active-drag-target"])).ok();

                data_transfer.set_effect_allowed("move");
                data_transfer
                    .set_data("text/html", &element.inner_html())
                    .ok();
                *captured_source_content.borrow_mut() = Some(target);
                *dragging.borrow_mut() = true;
            }
        }) as Box<dyn FnMut(_)>);

        register("dragstart", &drag_start, &node_list).ok();
        drag_start.forget();
    }
    {
        let node_list = _node_list.clone();
        let drag_enter = Closure::wrap(Box::new(move |event: DragEvent| {
            let target: web_sys::EventTarget = event.target().unwrap();
            let element: &Element = target.dyn_ref::<Element>().unwrap();
            let class_list: DomTokenList = element.class_list();
            class_list.add(&to_js_array(&["over"])).ok();
        }) as Box<dyn FnMut(_)>);

        register("dragenter", &drag_enter, &node_list).ok();
        drag_enter.forget();
    }
    {
        let node_list = _node_list.clone();
        let drag_over = Closure::wrap(Box::new(move |event: DragEvent| {
            event.prevent_default();
            let data_transfer: DataTransfer = event.data_transfer().unwrap();
            data_transfer.set_drop_effect("move");
        }) as Box<dyn FnMut(_)>);

        register("dragover", &drag_over, &node_list).ok();
        drag_over.forget();
    }
    {
        let node_list = _node_list.clone();
        let drag_leave = Closure::wrap(Box::new(move |event: DragEvent| {
            event.stop_propagation();

            let target: web_sys::EventTarget = event.target().unwrap();
            let element: &Element = target.dyn_ref::<Element>().unwrap();
            let class_list: DomTokenList = element.class_list();
            class_list.remove(&to_js_array(&["over"])).ok();
        }) as Box<dyn FnMut(_)>);

        register("dragleave", &drag_leave, &node_list).ok();
        drag_leave.forget();
    }
    {
        let node_list = _node_list.clone();
        let captured_source_content = drag_source_content.clone();
        let mut set_source_content = drag_source_content;
        let drag_drop = Closure::wrap(Box::new(move |event: DragEvent| {
            let data_transfer: DataTransfer = event.data_transfer().unwrap();
            let target: web_sys::EventTarget = event.target().unwrap();
            let element: &Element = target.dyn_ref::<Element>().unwrap();

            trace!("Pre drop");
            if element.class_list().contains("draggable") {
                debug!("Dropping on Draggable");
                if let Some(dropped_target) = &*captured_source_content.borrow() {
                    data_transfer.set_effect_allowed("move");

                    let dropped_element: &Element = dropped_target.dyn_ref::<Element>().unwrap();
                    dropped_element.set_inner_html(element.inner_html().as_str());
                    element.set_inner_html(data_transfer.get_data("text/html").unwrap().as_str());

                    info!("Dropped on Draggable");
                    let mut _content = Rc::make_mut(&mut set_source_content).borrow_mut().as_ref();
                    _content = None;
                }
            }
        }) as Box<dyn FnMut(_)>);

        register("drop", &drag_drop, &node_list).ok();
        drag_drop.forget();
    }
    {
        let node_list = _node_list.clone();
        let dragging = drag_active;
        let drag_end = Closure::wrap(Box::new(move |event: DragEvent| {
            let len = node_list.length();
            for i in 0..len {
                let node: Node = node_list.get(i).unwrap();
                let element: &Element = node.dyn_ref::<Element>().unwrap();
                let class_list: DomTokenList = element.class_list();
                class_list.remove(&to_js_array(&["over"])).ok();
            }

            let target: web_sys::EventTarget = event.target().unwrap();
            let element: &Element = target.dyn_ref::<Element>().unwrap();

            let class_list: DomTokenList = element.class_list();
            class_list
                .remove(&to_js_array(&["active-drag-target"]))
                .ok();
            *dragging.borrow_mut() = false;
        }) as Box<dyn FnMut(_)>);

        register("dragend", &drag_end, &_node_list).ok();
        drag_end.forget();
    }

    Ok(())
}

fn register(
    evt_name: &str,
    func: &Closure<dyn FnMut(DragEvent)>,
    node_list: &NodeList,
) -> Result<(), JsValue> {
    let len = node_list.length();
    for i in 0..len {
        let node: Node = node_list.get(i).unwrap();
        let el: &Element = node.dyn_ref::<Element>().unwrap();
        el.add_event_listener_with_callback(evt_name, func.as_ref().unchecked_ref())?;
    }

    Ok(())
}
