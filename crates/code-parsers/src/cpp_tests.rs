#[cfg(test)]
mod tests {
    use super::cpp::*;
    use crate::types::Element;

    #[test]
    fn test_parse_simple_function() {
        let content = r#"
int add(int a, int b) {
    return a + b;
}
"#;
        let elements = parse_cpp(content).unwrap();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].name, "add");
        assert_eq!(elements[0].element_type, "function");
        assert_eq!(elements[0].lang, "cpp");
    }

    #[test]
    fn test_parse_class_with_methods() {
        let content = r#"
class Calculator {
public:
    Calculator();
    ~Calculator();
    int add(int a, int b);
    static bool validate(int x);
    virtual void reset();
};
"#;
        let elements = parse_cpp(content).unwrap();
        
        // Should find the class and its methods
        assert!(elements.len() >= 5);
        
        let class_elem = elements.iter().find(|e| e.name == "Calculator" && e.element_type == "class");
        assert!(class_elem.is_some());
        
        let add_method = elements.iter().find(|e| e.name == "add" && e.element_type == "method");
        assert!(add_method.is_some());
    }

    #[test]
    fn test_parse_template_class() {
        let content = r#"
template<typename T>
class Vector {
private:
    T* data;
    size_t size;
    
public:
    Vector();
    ~Vector();
    void push_back(const T& item);
    T& operator[](size_t index);
};
"#;
        let elements = parse_cpp(content).unwrap();
        
        let class_elem = elements.iter().find(|e| e.name == "Vector" && e.element_type == "class");
        assert!(class_elem.is_some());
        
        let push_back = elements.iter().find(|e| e.name == "push_back");
        assert!(push_back.is_some());
        
        let operator_elem = elements.iter().find(|e| e.name == "operator[]");
        assert!(operator_elem.is_some());
    }

    #[test]
    fn test_parse_namespace() {
        let content = r#"
namespace math {
    int square(int x) {
        return x * x;
    }
    
    namespace geometry {
        double area_circle(double radius);
    }
}
"#;
        let elements = parse_cpp(content).unwrap();
        
        let namespace_elem = elements.iter().find(|e| e.name == "math" && e.element_type == "namespace");
        assert!(namespace_elem.is_some());
        
        let square_func = elements.iter().find(|e| e.name == "square");
        assert!(square_func.is_some());
        
        let nested_namespace = elements.iter().find(|e| e.name == "geometry" && e.element_type == "namespace");
        assert!(nested_namespace.is_some());
    }

    #[test]
    fn test_parse_template_function() {
        let content = r#"
template<typename T>
T max(T a, T b) {
    return (a > b) ? a : b;
}

template<class Iterator>
void sort(Iterator first, Iterator last) {
    // sorting implementation
}
"#;
        let elements = parse_cpp(content).unwrap();
        assert_eq!(elements.len(), 2);
        
        let max_func = elements.iter().find(|e| e.name == "max");
        assert!(max_func.is_some());
        
        let sort_func = elements.iter().find(|e| e.name == "sort");
        assert!(sort_func.is_some());
    }

    #[test]
    fn test_parse_operator_overloads() {
        let content = r#"
class Complex {
public:
    Complex operator+(const Complex& other) const;
    Complex& operator=(const Complex& other);
    bool operator==(const Complex& other) const;
    Complex operator++(int); // postfix
    Complex& operator++();   // prefix
};
"#;
        let elements = parse_cpp(content).unwrap();
        
        let class_elem = elements.iter().find(|e| e.name == "Complex" && e.element_type == "class");
        assert!(class_elem.is_some());
        
        // Check for operator overloads
        let plus_op = elements.iter().find(|e| e.name == "operator+");
        assert!(plus_op.is_some());
        
        let assign_op = elements.iter().find(|e| e.name == "operator=");
        assert!(assign_op.is_some());
        
        let equals_op = elements.iter().find(|e| e.name == "operator==");
        assert!(equals_op.is_some());
        
        let increment_ops: Vec<_> = elements.iter().filter(|e| e.name == "operator++").collect();
        assert_eq!(increment_ops.len(), 2); // prefix and postfix
    }

    #[test]
    fn test_parse_constructor_destructor() {
        let content = r#"
class MyClass {
public:
    MyClass();                          // default constructor
    MyClass(int value);                 // parameterized constructor
    MyClass(const MyClass& other);      // copy constructor
    MyClass(MyClass&& other) noexcept;  // move constructor
    ~MyClass();                         // destructor
};
"#;
        let elements = parse_cpp(content).unwrap();
        
        let class_elem = elements.iter().find(|e| e.name == "MyClass" && e.element_type == "class");
        assert!(class_elem.is_some());
        
        // Should find multiple constructors
        let constructors: Vec<_> = elements.iter().filter(|e| e.name == "MyClass" && e.element_type == "method").collect();
        assert!(constructors.len() >= 4);
        
        // Should find destructor
        let destructor = elements.iter().find(|e| e.name == "~MyClass");
        assert!(destructor.is_some());
    }

    #[test]
    fn test_parse_struct() {
        let content = r#"
struct Point {
    double x, y;
    
    Point(double x, double y) : x(x), y(y) {}
    
    double distance(const Point& other) const {
        return sqrt((x - other.x) * (x - other.x) + (y - other.y) * (y - other.y));
    }
};
"#;
        let elements = parse_cpp(content).unwrap();
        
        let struct_elem = elements.iter().find(|e| e.name == "Point" && e.element_type == "struct");
        assert!(struct_elem.is_some());
        
        let distance_method = elements.iter().find(|e| e.name == "distance");
        assert!(distance_method.is_some());
    }

    #[test]
    fn test_parse_enum() {
        let content = r#"
enum Color {
    RED,
    GREEN,
    BLUE
};

enum class Status : int {
    PENDING = 0,
    RUNNING = 1,
    COMPLETED = 2,
    FAILED = 3
};
"#;
        let elements = parse_cpp(content).unwrap();
        
        let color_enum = elements.iter().find(|e| e.name == "Color" && e.element_type == "enum");
        assert!(color_enum.is_some());
        
        let status_enum = elements.iter().find(|e| e.name == "Status" && e.element_type == "enum");
        assert!(status_enum.is_some());
    }

    #[test]
    fn test_parse_lambda_expressions() {
        let content = r#"
void test_lambdas() {
    auto lambda1 = [](int x) { return x * 2; };
    
    auto lambda2 = [&](const std::string& s) -> bool {
        return s.length() > 0;
    };
    
    std::sort(vec.begin(), vec.end(), [](int a, int b) {
        return a < b;
    });
}
"#;
        let elements = parse_cpp(content).unwrap();
        
        let test_func = elements.iter().find(|e| e.name == "test_lambdas");
        assert!(test_func.is_some());
        
        // Should capture function calls within the lambda context
        if let Some(func) = test_func {
            assert!(func.outgoing_calls.contains(&"sort".to_string()) || 
                   func.outgoing_calls.iter().any(|call| call.contains("sort")));
        }
    }

    #[test]
    fn test_parse_whitespace_only() {
        let content = "   \n\t  \n  ";
        let elements = parse_cpp(content).unwrap();
        assert_eq!(elements.len(), 0);
    }

    #[test]
    fn test_parse_empty_content() {
        let content = "";
        let elements = parse_cpp(content).unwrap();
        assert_eq!(elements.len(), 0);
    }

    #[test]
    fn test_parse_comments_only() {
        let content = r#"
// This is a comment
/* This is a multi-line
   comment */
/// Documentation comment
/**
 * Another documentation comment
 */
"#;
        let elements = parse_cpp(content).unwrap();
        assert_eq!(elements.len(), 0);
    }

    #[test]
    fn test_fallback_chunking() {
        // Content that might be hard to parse precisely
        let content = r#"
// Some complex macro usage that might confuse the parser
#define COMPLEX_MACRO(x, y) \
    do { \
        if (x > y) { \
            some_func(x); \
        } else { \
            other_func(y); \
        } \
    } while(0)

// Normal function that should be parsed
int simple_function() {
    return 42;
}

// More content that might be problematic
template<template<typename> class Container, typename T>
Container<T> complex_template_usage(Container<T>& container) {
    return container;
}
"#;
        let elements = parse_cpp(content).unwrap();
        
        // Should at least find the simple function
        let simple_func = elements.iter().find(|e| e.name == "simple_function");
        assert!(simple_func.is_some());
        
        // Should either parse the complex template or create fallback chunks
        assert!(!elements.is_empty());
    }

    #[test]
    fn test_no_overlapping_chunks() {
        let content = r#"
int func1() { return 1; }
int func2() { return 2; }
int func3() { return 3; }
"#;
        let elements = parse_cpp(content).unwrap();
        
        // Check that we don't have overlapping line ranges
        for i in 0..elements.len() {
            for j in (i+1)..elements.len() {
                let elem1 = &elements[i];
                let elem2 = &elements[j];
                
                // If both have line ranges, they shouldn't overlap
                if let (Some(start1), Some(end1), Some(start2), Some(end2)) = 
                   (elem1.start_line, elem1.end_line, elem2.start_line, elem2.end_line) {
                    assert!(end1 < start2 || end2 < start1, 
                           "Elements '{}' and '{}' have overlapping line ranges", 
                           elem1.name, elem2.name);
                }
            }
        }
    }

    #[test]
    fn test_cpp20_concepts() {
        let content = r#"
template<typename T>
concept Addable = requires(T a, T b) {
    a + b;
};

template<Addable T>
T add(T a, T b) {
    return a + b;
}
"#;
        let elements = parse_cpp(content).unwrap();
        
        // Should find the concept (might be parsed as a special kind of template)
        let concept_elem = elements.iter().find(|e| e.name == "Addable");
        assert!(concept_elem.is_some());
        
        // Should find the constrained template function
        let add_func = elements.iter().find(|e| e.name == "add");
        assert!(add_func.is_some());
    }

    #[test]
    fn test_cpp20_coroutines() {
        let content = r#"
#include <coroutine>

struct Task {
    struct promise_type {
        Task get_return_object() { return {}; }
        std::suspend_never initial_suspend() { return {}; }
        std::suspend_never final_suspend() noexcept { return {}; }
        void return_void() {}
        void unhandled_exception() {}
    };
};

Task async_function() {
    co_await something();
    co_return;
}
"#;
        let elements = parse_cpp(content).unwrap();
        
        // Should find the Task struct
        let task_struct = elements.iter().find(|e| e.name == "Task");
        assert!(task_struct.is_some());
        
        // Should find the async function
        let async_func = elements.iter().find(|e| e.name == "async_function");
        assert!(async_func.is_some());
    }
}