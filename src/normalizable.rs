use std::{
    io,
    path::{Component, Path, PathBuf},
};

use crate::pool::{Pooled, PATH_POOL};

pub trait Normalizable {
    fn normalized(&self, current_dir: &Path) -> io::Result<Pooled<PathBuf>>;
    fn is_normal(&self) -> bool;
}

impl<P: AsRef<Path>> Normalizable for P {
    fn normalized(&self, current_dir: &Path) -> io::Result<Pooled<PathBuf>> {
        let path = self.as_ref();
        let mut normal_path = PATH_POOL.new_item();

        if !path.is_absolute() {
            normal_path.push(current_dir);
        }

        for component in path.components() {
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
