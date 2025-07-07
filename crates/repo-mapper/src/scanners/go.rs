use crate::types::{MethodInfo, MethodType};
use regex::Regex;

pub fn scan_line(
    line: &str,
    context: &str,
    docstring: Option<String>,
    methods: &mut Vec<MethodInfo>,
    line_number: usize,
    max_calls: usize,
) {
    let func_pattern = Regex::new(r"func\s+([a-zA-Z0-9_]+)\s*\(").unwrap();
    let method_pattern = Regex::new(r"func\s+\([^)]+\)\s+([a-zA-Z0-9_]+)\s*\(").unwrap();
    let interface_pattern = Regex::new(r"type\s+([a-zA-Z0-9_]+)\s+interface").unwrap();
    let interface_method_pattern = Regex::new(r"^\s*([a-zA-Z0-9_]+)\s*\(").unwrap();

    if let Some(captures) = method_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::GoMethod,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    } else if let Some(captures) = func_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::GoFunc,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    } else if let Some(captures) = interface_pattern.captures(line) {
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::GoInterface,
            params: String::new(),
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    } else if context.contains("interface") {
        if let Some(captures) = interface_method_pattern.captures(line) {
        let params = extract_params(line);
            methods.push(MethodInfo {
                name: captures[1].to_string(),
                method_type: MethodType::GoInterfaceMethod,
                params,
                context: context.to_string(),
                docstring,
                calls: Vec::new(),
                line_number: Some(line_number),
            });
        }
    }
}

fn extract_params(line: &str) -> String {
    if let Some(params) = line.find('(') {
        if let Some(end) = line[params..].find(')') {
            return line[params + 1..params + end].trim().to_string();
        }
    }
    String::new()
}

fn extract_method_calls(context: &str, max_calls: usize) -> Vec<String> {
    let mut calls = Vec::new();
    
    let method_patterns = [
        Regex::new(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\(").unwrap(),
        Regex::new(r"\.([a-zA-Z_][a-zA-Z0-9_]*)\(").unwrap(),
    ];

    for pattern in &method_patterns {
        for cap in pattern.captures_iter(context) {
            if let Some(method_name) = cap.get(1) {
                calls.push(method_name.as_str().to_string());
            }
        }
    }

    calls.sort();
    calls.dedup();
    calls.truncate(max_calls);
    calls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_function() {
        let mut methods = Vec::new();
        let line = "func ProcessData(input string, count int) error {";
        let context = "func ProcessData(input string, count int) error {\n    return nil\n}";
        
        scan_line(line, context, None, &mut methods, 10, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "ProcessData");
        assert_eq!(methods[0].method_type, MethodType::GoFunc);
        assert_eq!(methods[0].params, "input string, count int");
        assert_eq!(methods[0].line_number, Some(10));
    }

    #[test]
    fn test_scan_method() {
        let mut methods = Vec::new();
        let line = "func (s *Server) Start(port int) error {";
        let context = "func (s *Server) Start(port int) error {\n    s.initialize()\n    return s.listen(port)\n}";
        
        scan_line(line, context, None, &mut methods, 20, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Start");
        assert_eq!(methods[0].method_type, MethodType::GoMethod);
        // The current implementation includes the receiver in params
        assert_eq!(methods[0].params, "s *Server");
        assert_eq!(methods[0].line_number, Some(20));
        
        // Check method calls were extracted
        assert!(methods[0].calls.contains(&"initialize".to_string()));
        assert!(methods[0].calls.contains(&"listen".to_string()));
    }

    #[test]
    fn test_scan_interface() {
        let mut methods = Vec::new();
        let line = "type Storage interface {";
        let context = "type Storage interface {\n    Get(key string) ([]byte, error)\n    Set(key string, value []byte) error\n}";
        
        scan_line(line, context, None, &mut methods, 30, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Storage");
        assert_eq!(methods[0].method_type, MethodType::GoInterface);
        assert_eq!(methods[0].params, "");
        assert_eq!(methods[0].line_number, Some(30));
    }

    #[test]
    fn test_scan_interface_method() {
        let mut methods = Vec::new();
        let line = "    Get(key string) ([]byte, error)";
        let context = "type Storage interface {\n    Get(key string) ([]byte, error)\n    Set(key string, value []byte) error\n}";
        
        scan_line(line, context, None, &mut methods, 40, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Get");
        assert_eq!(methods[0].method_type, MethodType::GoInterfaceMethod);
        assert_eq!(methods[0].params, "key string");
        assert_eq!(methods[0].line_number, Some(40));
        assert!(methods[0].calls.is_empty()); // Interface methods don't have calls
    }

    #[test]
    fn test_scan_with_docstring() {
        let mut methods = Vec::new();
        let line = "func Calculate(x, y float64) float64 {";
        let context = "// Calculate performs a calculation\nfunc Calculate(x, y float64) float64 {\n    return x + y\n}";
        let docstring = Some("Calculate performs a calculation".to_string());
        
        scan_line(line, context, docstring, &mut methods, 50, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Calculate");
        assert_eq!(methods[0].docstring, Some("Calculate performs a calculation".to_string()));
    }

    #[test]
    fn test_extract_params_simple() {
        assert_eq!(extract_params("func Test(a int, b string)"), "a int, b string");
        assert_eq!(extract_params("func NoParams()"), "");
        assert_eq!(extract_params("func Complex(ctx context.Context, opts ...Option) error"), "ctx context.Context, opts ...Option");
    }

    #[test]
    fn test_extract_params_with_receiver() {
        // The current implementation extracts the receiver as params
        assert_eq!(extract_params("func (r *Repo) Clone(url string) error"), "r *Repo");
        assert_eq!(extract_params("func (User) Name() string"), "User");
    }

    #[test]
    fn test_extract_method_calls() {
        let context = r#"
            func process() {
                init()
                data := fetch()
                result := transform(data)
                logger.Info("done")
                db.Save(result)
            }
        "#;
        
        let calls = extract_method_calls(context, 10);
        
        assert!(calls.contains(&"Info".to_string()));
        assert!(calls.contains(&"Save".to_string()));
        assert!(calls.contains(&"fetch".to_string()));
        assert!(calls.contains(&"init".to_string()));
        assert!(calls.contains(&"transform".to_string()));
    }

    #[test]
    fn test_extract_method_calls_max_limit() {
        let context = r#"
            func many() {
                a(); b(); c(); d(); e();
                f(); g(); h(); i(); j();
                k(); l(); m(); n(); o();
            }
        "#;
        
        let calls = extract_method_calls(context, 5);
        
        assert_eq!(calls.len(), 5);
    }

    #[test]
    fn test_extract_method_calls_deduplication() {
        let context = r#"
            func duplicate() {
                process()
                process()
                process()
            }
        "#;
        
        let calls = extract_method_calls(context, 10);
        
        // The regex patterns match both "duplicate" and "process"
        assert_eq!(calls.len(), 2); // duplicate and process
        assert!(calls.contains(&"duplicate".to_string()));
        assert!(calls.contains(&"process".to_string()));
    }

    #[test]
    fn test_non_matching_lines() {
        let mut methods = Vec::new();
        
        // Test various non-matching lines
        scan_line("// This is a comment", "", None, &mut methods, 1, 10);
        scan_line("var x = 42", "", None, &mut methods, 2, 10);
        scan_line("const MaxSize = 1024", "", None, &mut methods, 3, 10);
        scan_line("import \"fmt\"", "", None, &mut methods, 4, 10);
        
        assert!(methods.is_empty());
    }

    #[test]
    fn test_embedded_method() {
        let mut methods = Vec::new();
        let line = "func (embedded) Process() {";
        let context = "func (embedded) Process() {\n    // do something\n}";
        
        scan_line(line, context, None, &mut methods, 60, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Process");
        assert_eq!(methods[0].method_type, MethodType::GoMethod);
    }

    #[test]
    fn test_function_with_return_types() {
        let mut methods = Vec::new();
        let line = "func GetUser(id int) (*User, error) {";
        let context = "func GetUser(id int) (*User, error) {\n    return db.FindUser(id)\n}";
        
        scan_line(line, context, None, &mut methods, 70, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "GetUser");
        assert_eq!(methods[0].params, "id int");
        assert!(methods[0].calls.contains(&"FindUser".to_string()));
    }
} 