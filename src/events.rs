use std::{cell::RefCell, rc::Rc};

pub struct Event<T> {
    subscribers: Vec<Rc<RefCell<dyn Subscriber<T>>>>,
}

pub struct EventToken<T>
where
    T: FnOnce() -> (),
{
    unsub_function: T,
}

impl<T> Drop for EventToken<T>
where
    T: FnOnce() -> (),
{
    fn drop(&mut self) {
        (self.unsub_function)();
    }
}

pub trait Subscriber<T> {
    fn HandleEvent(&self, param: T);
}

impl<T, S: Subscriber<T>> Subscriber<T> for &S {
    fn HandleEvent(&self, param: T) {
        todo!()
    }
}

impl<T> Event<T> {
    pub fn subscribe(&self, subscriber: &dyn Subscriber<T>) {
        // self.subscribers.push(Rc::new(RefCell::new(subscriber)));
    }
}
