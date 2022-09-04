use super::{
    file_forbids_unsafe, has_unsafe_attributes, is_test_fn, is_test_mod,
    IncludeTests, RsFileMetrics,
};

use syn::{visit, Expr, ImplItemMethod, ItemFn, ItemImpl, ItemMod, ItemTrait};

pub struct GeigerSynVisitor {
    /// Count unsafe usage inside tests
    include_tests: IncludeTests,

    /// The resulting data from a single file scan.
    pub metrics: RsFileMetrics,

    /// The number of nested unsafe scopes that the GeigerSynVisitor are
    /// currently in. For example, if the visitor is inside an unsafe function
    /// and inside an unnecessary unsafe block inside that function, then this
    /// number should be 2. If the visitor is outside unsafe scopes, in a safe
    /// scope, this number should be 0.
    /// This is needed since unsafe scopes can be nested and we need to know
    /// when we leave the outmost unsafe scope and get back into a safe scope.
    unsafe_scopes: u32,
}

impl GeigerSynVisitor {
    pub fn new(include_tests: IncludeTests) -> Self {
        GeigerSynVisitor {
            include_tests,
            metrics: Default::default(),
            unsafe_scopes: 0,
        }
    }

    pub fn enter_unsafe_scope(&mut self) {
        self.unsafe_scopes += 1;
    }

    pub fn exit_unsafe_scope(&mut self) {
        self.unsafe_scopes -= 1;
    }
}

impl<'ast> visit::Visit<'ast> for GeigerSynVisitor {
    fn visit_expr(&mut self, i: &Expr) {
        // Total number of expressions of any type
        match i {
            Expr::Unsafe(i) => {
                self.enter_unsafe_scope();
                visit::visit_expr_unsafe(self, i);
                self.exit_unsafe_scope();
            }
            Expr::Path(_) | Expr::Lit(_) => {
                // Do not count. The expression `f(x)` should count as one
                // expression, not three.
            }
            other => {
                // TODO: Print something pretty here or gather the data for later
                // printing.
                // if self.verbosity == Verbosity::Verbose && self.unsafe_scopes > 0 {
                //     println!("{:#?}", other);
                // }
                self.metrics.counters.exprs.count(self.unsafe_scopes > 0);
                visit::visit_expr(self, other);
            }
        }
    }

    fn visit_file(&mut self, i: &'ast syn::File) {
        self.metrics.forbids_unsafe = file_forbids_unsafe(i);
        visit::visit_file(self, i);
    }

    fn visit_impl_item_method(&mut self, i: &ImplItemMethod) {
        if i.sig.unsafety.is_some() {
            self.enter_unsafe_scope()
        }
        self.metrics
            .counters
            .methods
            .count(i.sig.unsafety.is_some());
        visit::visit_impl_item_method(self, i);
        if i.sig.unsafety.is_some() {
            self.exit_unsafe_scope()
        }
    }

    /// Free-standing functions
    fn visit_item_fn(&mut self, item_fn: &ItemFn) {
        if IncludeTests::No == self.include_tests && is_test_fn(item_fn) {
            return;
        }
        let unsafe_fn =
            item_fn.sig.unsafety.is_some() || has_unsafe_attributes(item_fn);
        if unsafe_fn {
            self.enter_unsafe_scope()
        }
        self.metrics.counters.functions.count(unsafe_fn);
        visit::visit_item_fn(self, item_fn);
        if item_fn.sig.unsafety.is_some() {
            self.exit_unsafe_scope()
        }
    }

    fn visit_item_impl(&mut self, i: &ItemImpl) {
        // unsafe trait impl's
        self.metrics.counters.item_impls.count(i.unsafety.is_some());
        visit::visit_item_impl(self, i);
    }

    fn visit_item_mod(&mut self, i: &ItemMod) {
        if IncludeTests::No == self.include_tests && is_test_mod(i) {
            return;
        }
        visit::visit_item_mod(self, i);
    }

    fn visit_item_trait(&mut self, i: &ItemTrait) {
        // Unsafe traits
        self.metrics
            .counters
            .item_traits
            .count(i.unsafety.is_some());
        visit::visit_item_trait(self, i);
    }

    // TODO: Visit macros.
    //
    // TODO: Figure out if there are other visit methods that should be
    // implemented here.
}

#[cfg(test)]
mod geiger_syn_visitor_tests {
    use super::*;
    use cargo_geiger_serde::{Count, CounterBlock};
    use rstest::*;
    use syn::visit::Visit;
    use syn::{Expr, File};

    #[rstest(
        expression_str,
        expected_safe_exprs,
        expected_unsafe_exprs,
        case(
            "unsafe {
                let address = 0x01234usize;
                let r = address as *mut i32;
                std::slice::from_raw_parts_mut(r, 10000);
             }",
            0,
            2,
        ),
        case("unsafe { let x = 2; }", 0, 0,),
        case("let x = 2", 1, 0,)
    )]
    fn visit_expr_test(
        expression_str: &str,
        expected_safe_exprs: u64,
        expected_unsafe_exprs: u64,
    ) {
        let mut geiger_syn_visitor = GeigerSynVisitor::new(IncludeTests::Yes);
        let expr: Expr = syn::parse_str(expression_str).unwrap();
        geiger_syn_visitor.visit_expr(&expr);

        assert_eq!(
            geiger_syn_visitor.metrics.counters.exprs.safe,
            expected_safe_exprs,
        );

        assert_eq!(
            geiger_syn_visitor.metrics.counters.exprs.unsafe_,
            expected_unsafe_exprs,
        )
    }

    #[rstest(
        input_file_str,
        input_include_tests,
        expected_counter_block,
        case(
            "
                mod a { fn b() {3} }
                #[cfg(test)]
                mod c { fn d() {4} }
            ",
            IncludeTests::No,
            CounterBlock {
                functions: Count {
                    safe: 1,
                    unsafe_: 0
                },
                exprs: Count {
                    safe: 0,
                    unsafe_: 0
                },
                item_impls: Count {
                    safe: 0,
                    unsafe_: 0
                },
                item_traits: Count {
                    safe: 0,
                    unsafe_: 0
                },
                methods: Count {
                    safe: 0,
                    unsafe_: 0
                },
            }
        ),
        case(
            "
                mod a { fn b() {3} }
                #[cfg(test)]
                mod c { fn d() {4} }
            ",
            IncludeTests::Yes,
            CounterBlock {
                functions: Count {
                    safe: 2,
                    unsafe_: 0
                },
                exprs: Count {
                    safe: 0,
                    unsafe_: 0
                },
                item_impls: Count {
                    safe: 0,
                    unsafe_: 0
                },
                item_traits: Count {
                    safe: 0,
                    unsafe_: 0
                },
                methods: Count {
                    safe: 0,
                    unsafe_: 0
                },
            }
        ),
        case(
            "
                fn main() {
                    let address = 0x01234usize;
                    let r = address as *mut i32;
                    unsafe { std::slice::from_raw_parts_mut(r, 10000) }
                }
                mod a { fn b() {3} }
                #[cfg(test)]
                mod c { fn d() {4} }
            ",
            IncludeTests::Yes,
            CounterBlock {
                functions: Count {
                    safe: 3,
                    unsafe_: 0
                },
                exprs: Count {
                    safe: 1,
                    unsafe_: 1
                },
                item_impls: Count {
                    safe: 0,
                    unsafe_: 0
                },
                item_traits: Count {
                    safe: 0,
                    unsafe_: 0
                },
                methods: Count {
                    safe: 0,
                    unsafe_: 0
                },
            }
        ),
    )]
    fn visit_file_test(
        input_file_str: &str,
        input_include_tests: IncludeTests,
        expected_counter_block: CounterBlock,
    ) {
        let mut geiger_syn_visitor = GeigerSynVisitor::new(input_include_tests);
        let input_file: File = syn::parse_str(input_file_str).unwrap();
        geiger_syn_visitor.visit_file(&input_file);

        assert_eq!(geiger_syn_visitor.metrics.counters, expected_counter_block,)
    }
}
