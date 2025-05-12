use std::{
    io,
    path::{absolute, Component, Path, PathBuf},
};

use crate::pool::{Pooled, PATH_POOL};

pub trait Normalizable {
    fn normalized(&self) -> io::Result<Pooled<PathBuf>>;
    fn is_normal(&self) -> bool;
}

impl<P: AsRef<Path>> Normalizable for P {
    fn normalized(&self) -> io::Result<Pooled<PathBuf>> {
        let absolute_path = absolute(self)?;
        let mut normal_path = PATH_POOL.new_item();

        for component in absolute_path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normal_path.pop();
                }
                _ => {
                    normal_path.push(component.as_os_str());
                }
            }
        }

        Ok(normal_path)
    }

    fn is_normal(&self) -> bool {
        let path: &Path = self.as_ref();

        if !path.is_absolute() {
            return false;
        }

        path.components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir))
    }
}
