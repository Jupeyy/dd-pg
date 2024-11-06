/// in contrast to `compile_test_safe_rc` this tries to return a reference to r: String
/// which is impossible
fn main() {
    use hiarc::hiarc;
    use hiarc::hiarc_safer_rc_refcell;

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Default)]
    pub struct R {
        r: String,
    }

    #[hiarc_safer_rc_refcell]
    impl R {
        pub(crate) fn push_str(&mut self, s: &str) {
            self.r.push_str(s)
        }

        pub(crate) fn get(&mut self) -> &String {
            &self.r
        }
    }

    #[hiarc]
    pub struct A {
        r: R,
    }

    impl A {
        pub fn call_me(&self) {
            self.r.push_str("test A");
        }
    }

    #[hiarc]
    pub struct B {
        r: R,
        a: A,
    }

    impl B {
        pub fn call_me(&self) {
            let r = &self.r;
            self.a.call_me();
            r.push_str("test B");
        }
    }

    let r = R::default();
    let b = B {
        r: r.clone(),
        a: A { r: r.clone() },
    };
    b.call_me();
    println!("{}", r.get());
}
