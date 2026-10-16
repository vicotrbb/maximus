use send_wrapper::SendWrapper;
use std::{cell::RefCell, future::Future, pin::Pin, rc::Rc};
type Task = Pin<Box<dyn Future<Output = ()>>>;
/// One upgraded task per connection. Its lifetime remains inside connection admission and drain.
#[derive(Clone)]
pub struct UpgradeSlot(SendWrapper<Rc<RefCell<Option<Task>>>>);
impl Default for UpgradeSlot {
    fn default() -> Self {
        Self(SendWrapper::new(Rc::new(RefCell::new(None))))
    }
}
impl UpgradeSlot {
    pub fn register(&self, task: impl Future<Output = ()> + 'static) -> bool {
        let mut slot = self.0.borrow_mut();
        if slot.is_some() {
            return false;
        }
        *slot = Some(Box::pin(task));
        true
    }
    pub(crate) fn take(&self) -> Option<Task> {
        self.0.borrow_mut().take()
    }
}
