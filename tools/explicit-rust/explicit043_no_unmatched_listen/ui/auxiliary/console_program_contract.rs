pub enum Subscription {
    Once(u8),
    Timer(u8),
}

pub enum Effect {
    Subscribe(Subscription),
}
