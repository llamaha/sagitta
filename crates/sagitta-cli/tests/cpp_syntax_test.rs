use code_parsers::cpp::parse_cpp;

#[test]
fn test_cpp_parser_integration() {
    let content = r#"
/// A sample C++ class for testing
class TestClass {
private:
    int value;
    
public:
    /// Constructor
    TestClass(int val) : value(val) {}
    
    /// Destructor
    ~TestClass() {}
    
    /// Get the value
    int getValue() const { return value; }
    
    /// Set the value
    void setValue(int val) { value = val; }
    
    /// Static utility function
    static bool isValid(int val) {
        return val >= 0;
    }
    
    /// Virtual function for inheritance
    virtual void process() {
        // Implementation
    }
};

/// Free function outside class
template<typename T>
T maximum(T a, T b) {
    return (a > b) ? a : b;
}

/// Namespace with functions
namespace utilities {
    /// Calculate square
    int square(int x) {
        return x * x;
    }
    
    /// Nested namespace
    namespace math {
        double PI = 3.14159;
        
        double area_circle(double radius) {
            return PI * square(radius);
        }
    }
}
"#;

    let elements = parse_cpp(content).unwrap();
    
    // Should find multiple elements
    assert!(!elements.is_empty());
    
    // Check for class
    let class_elem = elements.iter().find(|e| e.name == "TestClass" && e.element_type == "class");
    assert!(class_elem.is_some(), "Should find TestClass");
    
    // Check for constructor
    let constructor = elements.iter().find(|e| e.name == "TestClass" && e.element_type == "method");
    assert!(constructor.is_some(), "Should find constructor");
    
    // Check for destructor
    let destructor = elements.iter().find(|e| e.name == "~TestClass");
    assert!(destructor.is_some(), "Should find destructor");
    
    // Check for template function
    let template_func = elements.iter().find(|e| e.name == "maximum");
    assert!(template_func.is_some(), "Should find template function");
    
    // Check for namespace
    let namespace_elem = elements.iter().find(|e| e.name == "utilities" && e.element_type == "namespace");
    assert!(namespace_elem.is_some(), "Should find utilities namespace");
    
    // Check for function in namespace
    let square_func = elements.iter().find(|e| e.name == "square");
    assert!(square_func.is_some(), "Should find square function");
    
    // Verify all elements have proper language tag
    for element in &elements {
        assert_eq!(element.lang, "cpp", "All elements should be tagged as C++");
    }
    
    // Verify identifiers are extracted
    for element in &elements {
        assert!(!element.identifiers.is_empty(), "Elements should have identifiers: {}", element.name);
    }
    
    println!("Successfully parsed {} C++ elements", elements.len());
    for element in &elements {
        println!("  - {} ({}): {}", element.name, element.element_type, element.identifiers.join(", "));
    }
}

#[test]
fn test_cpp_edge_cases() {
    let content = r#"
// Test various edge cases
template<template<typename> class Container, typename T>
class ComplexTemplate {
public:
    Container<T> data;
};

// Operator overloading
class Operators {
public:
    Operators operator+(const Operators& other) const;
    bool operator==(const Operators& other) const;
    Operators& operator++();  // prefix
    Operators operator++(int); // postfix
};

// Function pointers and lambdas
void test_lambdas() {
    auto lambda = [](int x) -> int { return x * 2; };
    std::function<int(int)> func = lambda;
}

// Macros and preprocessor directives
#define MAX(a, b) ((a) > (b) ? (a) : (b))

// Empty namespace
namespace empty {
}
"#;

    let elements = parse_cpp(content).unwrap();
    
    // Should handle complex templates
    let complex_template = elements.iter().find(|e| e.name == "ComplexTemplate");
    assert!(complex_template.is_some(), "Should handle complex templates");
    
    // Should handle operator overloads
    let operators_class = elements.iter().find(|e| e.name == "Operators" && e.element_type == "class");
    assert!(operators_class.is_some(), "Should find Operators class");
    
    // Should handle lambdas within functions
    let lambda_func = elements.iter().find(|e| e.name == "test_lambdas");
    assert!(lambda_func.is_some(), "Should find lambda test function");
    
    // Should handle empty constructs gracefully
    let empty_ns = elements.iter().find(|e| e.name == "empty");
    // May or may not be found depending on implementation, but shouldn't crash
    
    println!("Edge cases test passed with {} elements", elements.len());
}

#[test]
fn test_cpp_error_handling() {
    // Test malformed content
    let malformed_content = r#"
class Incomplete {
    // Missing closing brace and other issues
    int func(
"#;

    // Should not panic, might return empty or partial results
    let result = parse_cpp(malformed_content);
    assert!(result.is_ok(), "Parser should handle malformed content gracefully");
    
    // Test empty content
    let empty_result = parse_cpp("");
    assert!(empty_result.is_ok(), "Parser should handle empty content");
    assert_eq!(empty_result.unwrap().len(), 0, "Empty content should return no elements");
    
    // Test whitespace only
    let whitespace_result = parse_cpp("   \n\t  \n  ");
    assert!(whitespace_result.is_ok(), "Parser should handle whitespace-only content");
    assert_eq!(whitespace_result.unwrap().len(), 0, "Whitespace-only content should return no elements");
}