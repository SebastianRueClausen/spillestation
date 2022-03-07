use splst_cdimg::CdImage;

use std::rc::Rc;
use std::cell::{RefCell, Ref, RefMut};

#[derive(Clone, Default)]
pub struct Disc(Rc<RefCell<Option<CdImage>>>);

impl Disc {
    pub fn cd(&self) -> Ref<'_, Option<CdImage>>{
        self.0.borrow() 
    }

    pub fn cd_mut(&self) -> RefMut<'_, Option<CdImage>> {
        self.0.borrow_mut() 
    }

    pub fn is_loaded(&self) -> bool {
        self.0.borrow().is_some()
    }

    pub fn load(&self, cd: CdImage) {
        self.0.replace(Some(cd));
    }

    pub fn eject(&self) {
        *self.0.borrow_mut() = None;
    }
}
