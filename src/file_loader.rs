use async_std::{fs, path::PathBuf, task::block_on};
use crossbeam_channel::{Receiver, Sender};
use image::RgbaImage;
use rayon::ThreadPool;

const MAX_WORKER_COUNT: usize = 4;

pub struct LoadedFile {
    pub id: u32,
    pub loaded_image: RgbaImage,
}

pub struct FileLoader {
    next_resource_id: u32,
    thread_pool: ThreadPool,
    result_sender: Sender<anyhow::Result<LoadedFile>>,
    result_receiver: Receiver<anyhow::Result<LoadedFile>>,
}

impl FileLoader {
    pub fn new() -> Self {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_WORKER_COUNT)
            .build()
            .unwrap();
        let (result_sender, result_receiver) = crossbeam_channel::unbounded();

        FileLoader {
            next_resource_id: 0,
            thread_pool,
            result_sender,
            result_receiver,
        }
    }

    fn try_load_data(path: PathBuf) -> anyhow::Result<RgbaImage> {
        let data = block_on(fs::read(path))?;
        let img = image::load_from_memory(&data)?;
        Ok(img.to_rgba8())
    }

    pub fn start_loading_bytes(&mut self, path: &PathBuf) -> u32 {
        let resource_id = self.next_resource_id;
        self.next_resource_id += 1;
        let path = path.clone();
        let result_sender = self.result_sender.clone();

        // TODO: don't swallow the errors, propagate them
        self.thread_pool.spawn(move || {
            let image = Self::try_load_data(path).map(|image| LoadedFile {
                id: resource_id,
                loaded_image: image,
            });
            result_sender.send(image).unwrap();
        });

        resource_id
    }

    pub fn poll_loaded_resources(&self) -> Option<Vec<LoadedFile>> {
        let mut completed_resource_loads = Vec::new();
        while let Ok(result) = self.result_receiver.try_recv() {
            completed_resource_loads.push(result.unwrap());
        }

        if completed_resource_loads.is_empty() {
            None
        } else {
            Some(completed_resource_loads)
        }
    }
}
