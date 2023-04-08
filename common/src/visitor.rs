use super::message::Message;

pub trait Visitor {
    fn visit_twamp_sender(&mut self, message: impl Message);
}
