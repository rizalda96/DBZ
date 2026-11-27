//! Which images go to the AI. Only the latest user message's images are sent; earlier
//! ones are named in the text instead, so a long conversation doesn't resend (and pay
//! for) every picture on every reply.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::domain::{AiImage, ChatMsg, Role};

/// Index of the latest user message: the only one whose images are sent.
fn latest_user(msgs: &[ChatMsg]) -> Option<usize> {
    msgs.iter().rposition(|m| m.role == Role::User)
}

/// The images to send with this request.
pub(super) fn current_images(msgs: &[ChatMsg]) -> &[AiImage] {
    latest_user(msgs).map(|i| msgs[i].images.as_slice()).unwrap_or_default()
}

/// Whether message `i`'s images are sent with this request.
pub(super) fn sends_images(msgs: &[ChatMsg], i: usize) -> bool {
    latest_user(msgs) == Some(i) && !msgs[i].images.is_empty()
}

/// A line telling the AI which images belong to message `i`; None when it has none.
pub(super) fn image_note(msgs: &[ChatMsg], i: usize) -> Option<String> {
    let images = &msgs[i].images;
    if images.is_empty() {
        return None;
    }
    let names = images.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(", ");
    Some(if sends_images(msgs, i) {
        format!("[{} image(s) attached to this message: {}]", images.len(), names)
    } else {
        format!("[{} image(s) were attached here earlier and are not shown again: {}]", images.len(), names)
    })
}

pub(super) fn base64(data: &[u8]) -> String {
    STANDARD.encode(data)
}
