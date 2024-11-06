/// in contrast to `compile_test_unsafe_rc`
/// this does not even compile, so no runtime check
fn main() {
    use hiarc::hiarc;
    use hiarc::hiarc_safer_rc_refcell;

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Default)]
    pub struct R {
        r: String,
    }

    impl R {
        fn borrow_mut(&self) -> std::cell::RefMut<RImpl> {
            self.0.borrow_mut()
        }
    }

    #[hiarc]
    pub struct A {
        r: R,
    }

    impl A {
        pub fn call_me(&self) {
            let mut r = self.r.borrow_mut();
            r.r.push_str("test A");
        }
    }

    #[hiarc]
    pub struct B {
        r: R,
        a: A,
    }

    impl B {
        pub fn call_me(&self) {
            let mut r = self.r.borrow_mut();
            self.a.call_me();
            r.r.push_str("test B");
        }
    }

    let r = R::default();
    let b = B {
        r: r.clone(),
        a: A { r: r.clone() },
    };
    b.call_me();
    println!("{}", r.borrow_mut().r);
}
