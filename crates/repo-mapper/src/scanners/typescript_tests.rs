#[cfg(test)]
mod tests {
    use super::super::scan_line;
    use crate::types::MethodType;

    #[test]
    fn test_scan_function_declaration() {
        let mut methods = Vec::new();
        scan_line(
            "function calculateTotal(items: Item[]): number {",
            "function calculateTotal(items: Item[]): number {\n  return items.reduce((sum, item) => sum + item.price, 0);\n}",
            None,
            &mut methods,
            1,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "calculateTotal");
        assert_eq!(methods[0].method_type, MethodType::TsFunction);
        assert_eq!(methods[0].params, "items: Item[]");
        assert_eq!(methods[0].line_number, Some(1));
    }

    #[test]
    fn test_scan_async_function() {
        let mut methods = Vec::new();
        scan_line(
            "async function fetchData(url: string): Promise<Data> {",
            "async function fetchData(url: string): Promise<Data> {\n  const response = await fetch(url);\n  return response.json();\n}",
            Some("Fetches data from API".to_string()),
            &mut methods,
            5,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "fetchData");
        assert_eq!(methods[0].method_type, MethodType::TsFunction);
        assert_eq!(methods[0].params, "url: string");
        assert_eq!(methods[0].docstring, Some("Fetches data from API".to_string()));
        assert!(methods[0].calls.contains(&"fetch".to_string()));
        assert!(methods[0].calls.contains(&"json".to_string()));
    }

    #[test]
    fn test_scan_arrow_function() {
        let mut methods = Vec::new();
        scan_line(
            "const processData = (data: string) => {",
            "const processData = (data: string) => {\n  return data.trim().toUpperCase();\n}",
            None,
            &mut methods,
            3,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "processData");
        assert_eq!(methods[0].method_type, MethodType::TsArrow);
        assert_eq!(methods[0].params, "data: string");
        assert!(methods[0].calls.contains(&"trim".to_string()));
        assert!(methods[0].calls.contains(&"toUpperCase".to_string()));
    }

    #[test]
    fn test_scan_async_arrow_function() {
        let mut methods = Vec::new();
        scan_line(
            "let getData = async () => {",
            "let getData = async () => {\n  return await api.get('/data');\n}",
            None,
            &mut methods,
            7,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "getData");
        assert_eq!(methods[0].method_type, MethodType::TsArrow);
        assert_eq!(methods[0].params, "");
        assert!(methods[0].calls.contains(&"get".to_string()));
    }

    #[test]
    fn test_scan_class_declaration() {
        let mut methods = Vec::new();
        scan_line(
            "class UserService {",
            "class UserService {\n  constructor(private db: Database) {}\n}",
            Some("Service for user operations".to_string()),
            &mut methods,
            10,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "UserService");
        assert_eq!(methods[0].method_type, MethodType::TsClass);
        assert_eq!(methods[0].params, "");
        assert_eq!(methods[0].docstring, Some("Service for user operations".to_string()));
    }

    #[test]
    fn test_scan_class_method() {
        let mut methods = Vec::new();
        scan_line(
            "  public async getUserById(id: string): Promise<User> {",
            "  public async getUserById(id: string): Promise<User> {\n    return this.db.findOne({ id });\n  }",
            None,
            &mut methods,
            15,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "getUserById");
        assert_eq!(methods[0].method_type, MethodType::TsMethod);
        assert_eq!(methods[0].params, "id: string");
        assert!(methods[0].calls.contains(&"findOne".to_string()));
    }

    #[test]
    fn test_scan_private_method() {
        let mut methods = Vec::new();
        scan_line(
            "  private validateData(data: any): boolean {",
            "  private validateData(data: any): boolean {\n    return data !== null && data !== undefined;\n  }",
            None,
            &mut methods,
            20,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "validateData");
        assert_eq!(methods[0].method_type, MethodType::TsMethod);
        assert_eq!(methods[0].params, "data: any");
    }

    #[test]
    fn test_scan_interface_declaration() {
        let mut methods = Vec::new();
        scan_line(
            "interface User {",
            "interface User {\n  id: string;\n  name: string;\n  email: string;\n}",
            Some("User interface".to_string()),
            &mut methods,
            25,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "User");
        assert_eq!(methods[0].method_type, MethodType::TsInterface);
        assert_eq!(methods[0].params, "");
        assert_eq!(methods[0].docstring, Some("User interface".to_string()));
    }

    #[test]
    fn test_scan_type_alias() {
        let mut methods = Vec::new();
        scan_line(
            "type Status = 'active' | 'inactive' | 'pending';",
            "type Status = 'active' | 'inactive' | 'pending';",
            None,
            &mut methods,
            30,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Status");
        assert_eq!(methods[0].method_type, MethodType::TsType);
        assert_eq!(methods[0].params, "");
    }

    #[test]
    fn test_scan_complex_type() {
        let mut methods = Vec::new();
        // The regex doesn't match type declarations that end with {
        // It only matches simple type aliases like "type X ="
        scan_line(
            "type ApiResponse = { data: any; error?: string; status: number };",
            "type ApiResponse = { data: any; error?: string; status: number };",
            Some("Generic API response type".to_string()),
            &mut methods,
            35,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "ApiResponse");
        assert_eq!(methods[0].method_type, MethodType::TsType);
        assert_eq!(methods[0].docstring, Some("Generic API response type".to_string()));
    }

    #[test]
    fn test_extract_params() {
        let mut methods = Vec::new();
        
        // Test multiple parameters
        scan_line(
            "function add(a: number, b: number): number {",
            "function add(a: number, b: number): number { return a + b; }",
            None,
            &mut methods,
            40,
            10,
        );
        assert_eq!(methods[0].params, "a: number, b: number");
        
        // Test no parameters
        methods.clear();
        scan_line(
            "function getCurrentTime(): Date {",
            "function getCurrentTime(): Date { return new Date(); }",
            None,
            &mut methods,
            42,
            10,
        );
        assert_eq!(methods[0].params, "");
        
        // Test complex parameters
        methods.clear();
        scan_line(
            "function process(options: { timeout: number; retry: boolean }): void {",
            "function process(options: { timeout: number; retry: boolean }): void {}",
            None,
            &mut methods,
            44,
            10,
        );
        assert_eq!(methods[0].params, "options: { timeout: number; retry: boolean }");
    }

    #[test]
    fn test_extract_method_calls() {
        let mut methods = Vec::new();
        
        let context = r#"
        function processUser(user: User) {
            validateUser(user);
            const normalized = normalizeData(user);
            db.save(normalized);
            logger.info('User processed');
            return sendNotification(user.email);
        }
        "#;
        
        scan_line(
            "function processUser(user: User) {",
            context,
            None,
            &mut methods,
            50,
            10,
        );
        
        assert!(methods[0].calls.contains(&"validateUser".to_string()));
        assert!(methods[0].calls.contains(&"normalizeData".to_string()));
        assert!(methods[0].calls.contains(&"save".to_string()));
        assert!(methods[0].calls.contains(&"info".to_string()));
        assert!(methods[0].calls.contains(&"sendNotification".to_string()));
    }

    #[test]
    fn test_max_calls_limit() {
        let mut methods = Vec::new();
        
        let context = r#"
        function complexFunction() {
            a(); b(); c(); d(); e();
            f(); g(); h(); i(); j();
            k(); l(); m(); n(); o();
        }
        "#;
        
        scan_line(
            "function complexFunction() {",
            context,
            None,
            &mut methods,
            60,
            5, // max_calls = 5
        );
        
        assert_eq!(methods[0].calls.len(), 5);
    }

    #[test]
    fn test_no_match() {
        let mut methods = Vec::new();
        
        // Regular variable declaration
        scan_line(
            "const name = 'John';",
            "const name = 'John';",
            None,
            &mut methods,
            70,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // Import statement
        scan_line(
            "import { Component } from '@angular/core';",
            "import { Component } from '@angular/core';",
            None,
            &mut methods,
            71,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // Comment
        scan_line(
            "// This is a comment",
            "// This is a comment",
            None,
            &mut methods,
            72,
            10,
        );
        assert_eq!(methods.len(), 0);
    }

    #[test]
    fn test_arrow_function_variations() {
        let mut methods = Vec::new();
        
        // Arrow function with implicit return - the regex requires parentheses or braces
        scan_line(
            "const double = (x: number) => {",
            "const double = (x: number) => { return x * 2; }",
            None,
            &mut methods,
            80,
            10,
        );
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "double");
        assert_eq!(methods[0].params, "x: number");
        
        // Arrow function without type annotations but with parentheses
        methods.clear();
        scan_line(
            "const square = (x) => {",
            "const square = (x) => { return x * x; }",
            None,
            &mut methods,
            82,
            10,
        );
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "square");
        assert_eq!(methods[0].params, "x");
    }

    #[test]
    fn test_protected_method() {
        let mut methods = Vec::new();
        scan_line(
            "  protected handleError(error: Error): void {",
            "  protected handleError(error: Error): void {\n    console.error(error);\n  }",
            None,
            &mut methods,
            90,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "handleError");
        assert_eq!(methods[0].method_type, MethodType::TsMethod);
        assert_eq!(methods[0].params, "error: Error");
        assert!(methods[0].calls.contains(&"error".to_string()));
    }

    #[test]
    fn test_method_without_visibility_modifier() {
        let mut methods = Vec::new();
        scan_line(
            "  render(): JSX.Element {",
            "  render(): JSX.Element {\n    return <div>Hello</div>;\n  }",
            None,
            &mut methods,
            95,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "render");
        assert_eq!(methods[0].method_type, MethodType::TsMethod);
        assert_eq!(methods[0].params, "");
    }
}