use rumqtt::Notification;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ErrorKind {
    #[error("Topic too short")]
    TopicTooShort,
    #[error("Not a tuya Topic")]
    NotATuyaTopic,
    #[error("Received unhandled notification `{0:?}`")]
    UnhandledNotification(Notification),
}
