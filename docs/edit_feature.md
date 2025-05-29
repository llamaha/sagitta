# Code Editing with sagitta-cli

The sagitta-cli tool provides powerful code editing capabilities that leverage its semantic understanding of code. You can perform targeted edits with validation to ensure changes are applied correctly and safely.

## Core Editing Concepts

The edit feature supports two key targeting mechanisms:

### 1. Semantic Element Targeting

Use element-based targeting when you want to replace an entire logical code unit like a class, function, or method:

```bash
sagitta-cli edit apply --file path/to/file.py --element "class:MyClass" --content-file new_class.py
```

This approach uses semantic understanding to identify the boundaries of the specified code element and replaces the entire element with new content.

### 2. Line-Based Targeting

Use line-based targeting for more precise edits when you want to:
- Insert new code at specific positions
- Modify portions of functions or classes
- Add methods to existing classes
- Make targeted changes to specific sections

```bash
sagitta-cli edit apply --file path/to/file.py --line-start 15 --line-end 20 --content "def new_method(self):\n    return True"
```

Line-based targeting requires careful consideration of the indentation patterns in the target file to ensure consistent formatting.

## Validation First Approach

For reliability and safety, the edit feature supports a validation-first workflow:

```bash
# First validate the edit
sagitta-cli edit validate --file path/to/file.py --element "function:process_data" --content-file new_function.py

# Then apply if validation succeeds
sagitta-cli edit apply --file path/to/file.py --element "function:process_data" --content-file new_function.py
```

Validation checks ensure:
- The target file exists and is readable/writable
- The specified element or line range exists
- The new content has valid syntax
- The edit can be applied safely

## Best Practices for Reliable Editing

For the most reliable editing workflows:

1. **Choose the right targeting approach**:
   - Use semantic targeting when you want to replace entire elements
   - Use line-based targeting for more granular changes

2. **Maintain consistent indentation**:
   - When using line-based targeting, ensure your content matches the indentation pattern of the surrounding code
   - Be especially careful when inserting methods or functions within a class

3. **Create properly formatted content files**:
   - Store complex edits in separate files with proper formatting
   - Ensure your content files use the same line ending style as the target file

4. **Validate before applying**:
   - Always validate edits before applying them, especially for critical code
   - Check for any warnings in the validation output

5. **Test after editing**:
   - Run appropriate tests after applying edits to verify functionality
   - Consider using version control to track changes

## Command Reference

### Validate Command

```
sagitta-cli edit validate [OPTIONS] --file <FILE>

Options:
  --file <FILE>             Path to the file to validate against
  --line-start <LINE_START> Starting line number for the edit (1-based, inclusive)
  --line-end <LINE_END>     Ending line number for the edit (1-based, inclusive)
  --element <ELEMENT>       Semantic element to target (e.g., "function:my_func")
  --content-file <CONTENT_FILE> Path to a file containing the new content
  --content <CONTENT>       Inline content for the edit
  --format                  Automatically format the edited code block
  --update-references       Automatically update references to the edited element
  -h, --help                Print help
```

### Apply Command

```
sagitta-cli edit apply [OPTIONS] --file <FILE>

Options:
  --file <FILE>             Path to the file to edit
  --line-start <LINE_START> Starting line number for the edit (1-based, inclusive)
  --line-end <LINE_END>     Ending line number for the edit (1-based, inclusive)
  --element <ELEMENT>       Semantic element to target (e.g., "function:my_func")
  --content-file <CONTENT_FILE> Path to a file containing the new content
  --content <CONTENT>       Inline content for the edit
  --format                  Automatically format the edited code block
  --update-references       Automatically update references to the edited element
  -h, --help                Print help
```

## Element Targeting Syntax

The `--element` parameter uses a simple syntax to target specific code elements:

- `class:ClassName` - Target a class
- `function:function_name` - Target a function
- `method:ClassName.method_name` - Target a method within a class
- `struct:StructName` - Target a struct (in languages like Rust)
- `impl:StructName` - Target an implementation block (in Rust)