#[cfg(test)]
mod tests {
    use super::super::scan_line;
    use crate::types::MethodType;

    #[test]
    fn test_scan_vue_component_name() {
        let mut methods = Vec::new();
        scan_line(
            "  name: 'UserProfile',",
            "export default {\n  name: 'UserProfile',\n  props: ['userId']\n}",
            None,
            &mut methods,
            2,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "UserProfile");
        assert_eq!(methods[0].method_type, MethodType::VueComponent);
        assert_eq!(methods[0].line_number, Some(2));
    }

    #[test]
    fn test_scan_vue_component_name_double_quotes() {
        let mut methods = Vec::new();
        scan_line(
            r#"  name: "TodoList","#,
            r#"export default { name: "TodoList", data() { return { items: [] } } }"#,
            Some("Todo list component".to_string()),
            &mut methods,
            1,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "TodoList");
        assert_eq!(methods[0].method_type, MethodType::VueComponent);
        assert_eq!(methods[0].docstring, Some("Todo list component".to_string()));
    }

    #[test]
    fn test_scan_vue_method() {
        let mut methods = Vec::new();
        let context = r#"
        methods: {
          fetchUser(userId) {
            this.loading = true;
            api.getUser(userId).then(user => {
              this.user = user;
              this.loading = false;
            });
          }
        }
        "#;
        
        scan_line(
            "    fetchUser(userId) {",
            context,
            None,
            &mut methods,
            3,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "fetchUser");
        assert_eq!(methods[0].method_type, MethodType::VueMethod);
        assert_eq!(methods[0].params, "userId");
        assert!(methods[0].calls.contains(&"getUser".to_string()));
        assert!(methods[0].calls.contains(&"then".to_string()));
    }

    #[test]
    fn test_scan_vue_method_async() {
        let mut methods = Vec::new();
        let context = r#"
        methods: {
          async saveData() {
            try {
              await api.save(this.data);
              this.showSuccess();
            } catch (error) {
              this.handleError(error);
            }
          }
        }
        "#;
        
        scan_line(
            "    async saveData() {",
            context,
            Some("Saves data to server".to_string()),
            &mut methods,
            3,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "saveData");
        assert_eq!(methods[0].method_type, MethodType::VueMethod);
        assert_eq!(methods[0].params, "");
        assert_eq!(methods[0].docstring, Some("Saves data to server".to_string()));
        assert!(methods[0].calls.contains(&"save".to_string()));
        assert!(methods[0].calls.contains(&"showSuccess".to_string()));
        assert!(methods[0].calls.contains(&"handleError".to_string()));
    }

    #[test]
    fn test_scan_vue_computed() {
        let mut methods = Vec::new();
        let context = r#"
        computed: {
          fullName() {
            return this.firstName + ' ' + this.lastName;
          },
          isValid() {
            return this.email && this.password.length >= 8;
          }
        }
        "#;
        
        scan_line(
            "    fullName() {",
            context,
            None,
            &mut methods,
            3,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "fullName");
        assert_eq!(methods[0].method_type, MethodType::VueComputed);
        assert_eq!(methods[0].params, "");
    }

    #[test]
    fn test_scan_vue_computed_with_getter() {
        let mut methods = Vec::new();
        let context = r#"
        computed: {
          total() {
            return this.items.reduce((sum, item) => sum + item.price, 0);
          }
        }
        "#;
        
        scan_line(
            "    total() {",
            context,
            Some("Calculates total price".to_string()),
            &mut methods,
            3,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "total");
        assert_eq!(methods[0].method_type, MethodType::VueComputed);
        assert_eq!(methods[0].docstring, Some("Calculates total price".to_string()));
        assert!(methods[0].calls.contains(&"reduce".to_string()));
    }

    #[test]
    fn test_scan_vue_prop() {
        let mut methods = Vec::new();
        let context = r#"
        props: {
          userId: {
            type: String,
            required: true
          },
          showHeader: {
            type: Boolean,
            default: true
          }
        }
        "#;
        
        scan_line(
            "    userId: {",
            context,
            None,
            &mut methods,
            3,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "userId");
        assert_eq!(methods[0].method_type, MethodType::VueProp);
        assert_eq!(methods[0].params, "");
    }

    #[test]
    fn test_scan_vue_prop_with_docstring() {
        let mut methods = Vec::new();
        let context = "props: { maxItems: { type: Number, default: 10 } }";
        
        scan_line(
            "    maxItems: {",
            context,
            Some("Maximum number of items to display".to_string()),
            &mut methods,
            2,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "maxItems");
        assert_eq!(methods[0].method_type, MethodType::VueProp);
        assert_eq!(methods[0].docstring, Some("Maximum number of items to display".to_string()));
    }

    #[test]
    fn test_scan_method_with_multiple_params() {
        let mut methods = Vec::new();
        let context = r#"
        methods: {
          updateUser(id, name, email) {
            api.updateUser(id, { name, email });
          }
        }
        "#;
        
        scan_line(
            "    updateUser(id, name, email) {",
            context,
            None,
            &mut methods,
            3,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "updateUser");
        assert_eq!(methods[0].params, "id, name, email");
        assert!(methods[0].calls.contains(&"updateUser".to_string()));
    }

    #[test]
    fn test_scan_method_with_destructured_params() {
        let mut methods = Vec::new();
        let context = r#"
        methods: {
          handleSubmit({ data, options }) {
            this.process(data, options);
          }
        }
        "#;
        
        scan_line(
            "    handleSubmit({ data, options }) {",
            context,
            None,
            &mut methods,
            3,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "handleSubmit");
        assert_eq!(methods[0].params, "{ data, options }");
    }

    #[test]
    fn test_extract_method_calls_deduplication() {
        let mut methods = Vec::new();
        let context = r#"
        methods: {
          processData() {
            validate(this.data);
            validate(this.options);
            format(this.data);
            validate(this.result);
          }
        }
        "#;
        
        scan_line(
            "    processData() {",
            context,
            None,
            &mut methods,
            3,
            10,
        );
        
        // validate should only appear once due to deduplication
        assert_eq!(methods[0].calls.iter().filter(|&c| c == "validate").count(), 1);
        assert!(methods[0].calls.contains(&"format".to_string()));
    }

    #[test]
    fn test_max_calls_limit() {
        let mut methods = Vec::new();
        let context = r#"
        methods: {
            complexMethod() {
                a(); b(); c(); d(); e();
                f(); g(); h(); i(); j();
            }
        }
        "#;
        
        scan_line(
            "        complexMethod() {",
            context,
            None,
            &mut methods,
            3,
            5, // max_calls = 5
        );
        
        assert_eq!(methods[0].calls.len(), 5);
    }

    #[test]
    fn test_no_match_outside_context() {
        let mut methods = Vec::new();
        
        // Method-like line but not in methods context
        scan_line(
            "  doSomething() {",
            "data() { return { count: 0 } }",
            None,
            &mut methods,
            5,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // Computed-like line but not in computed context
        scan_line(
            "  calculate() {",
            "mounted() { this.init(); }",
            None,
            &mut methods,
            5,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // Prop-like line but not in props context
        scan_line(
            "  value: {",
            "data() { return { value: null } }",
            None,
            &mut methods,
            5,
            10,
        );
        assert_eq!(methods.len(), 0);
    }

    #[test]
    fn test_scan_nested_object_method() {
        let mut methods = Vec::new();
        let context = r#"
        methods: {
          handlers: {
            onClick() {
              this.handleClick();
            }
          }
        }
        "#;
        
        // Even though it's in methods context, the pattern should match
        scan_line(
            "      onClick() {",
            context,
            None,
            &mut methods,
            4,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "onClick");
        assert_eq!(methods[0].method_type, MethodType::VueMethod);
    }

    #[test]
    fn test_scan_arrow_function_method() {
        let mut methods = Vec::new();
        let context = "methods: { getData: () => { return this.data; } }";
        
        // Arrow functions won't match the method pattern
        scan_line(
            "  getData: () => {",
            context,
            None,
            &mut methods,
            2,
            10,
        );
        
        assert_eq!(methods.len(), 0);
    }
}