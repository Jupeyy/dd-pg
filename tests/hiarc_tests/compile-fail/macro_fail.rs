fn main() {
    use hiarc::hiarc;
    use hiarc::HiBox;
    #[hiarc]
    pub struct A {
        #[hiarc(inner)]
        b: Option<HiBox<B>>,
    }

    #[hiarc]
    pub struct B {
        #[hiarc(inner)]
        b: Option<HiBox<A>>,
    }

    fn main() {
        let _ = A { b: None };
    }
}
