use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use jni::{
    JavaVM,
    vm::{AttachConfig, ScopeToken},
};
use url::Url;

use super::{api::AndroidHiddenApi, intent};

pub struct MediaFile {
    pub path: String,
    pub mime: String,
    pub name: Option<String>,
}

pub enum Action {
    Reply {
        channel_id: i64,
        message: String,
        thread_id: Option<i64>,
    },
    React {
        channel_id: i64,
        message_id: i64,
    },
    MarkRead {
        channel_id: i64,
    },
    SendMedia {
        channel_id: i64,
        files: Vec<MediaFile>,
        multiple: bool,
    },
    EnterChannel {
        channel_id: i64,
    },
}

#[derive(Clone)]
pub struct ActionProcessor {
    sender: mpsc::Sender<Action>,
}

impl ActionProcessor {
    pub fn spawn(jvm: JavaVM, uid: Option<i32>, calling_package: String, referer: String) -> Self {
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let mut scope = ScopeToken::default();
            let mut guard = unsafe {
                jvm.attach_current_thread_guard(AttachConfig::default, &mut scope)
                    .expect("failed to attach Android action thread")
            };
            let api = AndroidHiddenApi::new(guard.borrow_env_mut(), uid, &calling_package)
                .expect("failed to create Android API");

            while let Ok(action) = receiver.recv() {
                if let Err(error) = Self::process(guard.borrow_env_mut(), &api, &referer, action) {
                    eprintln!("Android action failed: {error:?}");
                }
            }
        });

        Self { sender }
    }

    pub fn enqueue(&self, action: Action) -> Result<(), String> {
        self.sender
            .send(action)
            .map_err(|_| "Android action processor stopped".to_string())
    }

    fn process(
        env: &mut jni::Env<'_>,
        api: &AndroidHiddenApi,
        referer: &str,
        action: Action,
    ) -> jni::errors::Result<()> {
        match action {
            Action::Reply {
                channel_id,
                message,
                thread_id,
            } => {
                let intent = intent::reply(env, referer, channel_id, &message, thread_id)?;
                api.start_service(env, intent)?;
            }
            Action::React {
                channel_id,
                message_id,
            } => {
                let intent = intent::react(env, referer, channel_id, message_id)?;
                api.start_service(env, intent)?;
            }
            Action::MarkRead { channel_id } => {
                let intent = intent::mark_read(env, referer, channel_id)?;
                api.start_service(env, intent)?;
            }
            Action::EnterChannel { channel_id } => {
                let intent = intent::enter_channel(env, channel_id)?;
                api.start_activity(env, intent)?;
            }
            Action::SendMedia {
                channel_id,
                files,
                multiple,
            } => {
                let uris: Vec<_> = files.iter().map(content_uri).collect();
                if multiple {
                    let intent = intent::send_media_multiple(env, channel_id, &uris)?;
                    api.start_activity(env, intent)?;
                } else {
                    for (index, (uri, file)) in uris.iter().zip(&files).enumerate() {
                        if index > 0 {
                            thread::sleep(Duration::from_millis(100));
                        }
                        let intent = intent::send_media(env, channel_id, uri, &file.mime)?;
                        api.start_activity(env, intent)?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn content_uri(file: &MediaFile) -> String {
    let mut url = Url::parse("content://io.zugu.fileprovider").expect("valid content URI");
    url.set_path(&file.path);
    if let Some(name) = file.name.as_deref() {
        url.query_pairs_mut().append_pair("name", name);
    }
    url.query_pairs_mut().append_pair("mimeType", &file.mime);
    url.to_string()
}
