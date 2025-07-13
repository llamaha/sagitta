// Test to detect overlapping chunks across different language parsers

use code_parsers::{get_chunks, CodeChunk};
use std::path::Path;
use std::fs;

fn check_overlaps(chunks: &[CodeChunk]) -> Vec<(usize, usize)> {
    let mut overlaps = Vec::new();
    
    for (i, chunk1) in chunks.iter().enumerate() {
        for (j, chunk2) in chunks.iter().enumerate().skip(i + 1) {
            // Check if chunks overlap (same file and overlapping line ranges)
            if chunk1.file_path == chunk2.file_path {
                if chunk1.start_line <= chunk2.end_line && chunk2.start_line <= chunk1.end_line {
                    overlaps.push((i, j));
                }
            }
        }
    }
    
    overlaps
}

#[test]
fn test_no_overlaps_in_rust_parser() {
    let test_content = r#"
// Test file for overlap detection
use std::collections::HashMap;

/// A simple struct
struct Point {
    x: i32,
    y: i32,
}

/// Implementation block
impl Point {
    /// Constructor
    fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }
    
    /// Calculate distance
    fn distance(&self) -> f64 {
        ((self.x * self.x + self.y * self.y) as f64).sqrt()
    }
}

/// Main function
fn main() {
    let p = Point::new(3, 4);
    println!("Distance: {}", p.distance());
}

/// Another function
fn helper() {
    println!("Helper");
}

// Nested modules to test complex cases
mod outer {
    mod inner {
        fn nested_func() {}
    }
}
"#;
    
    let temp_file = "/tmp/test_overlap_rust.rs";
    fs::write(temp_file, test_content).unwrap();
    
    let chunks = get_chunks(Path::new(temp_file)).unwrap();
    let overlaps = check_overlaps(&chunks);
    
    if !overlaps.is_empty() {
        println!("Found {} overlaps in Rust parser:", overlaps.len());
        for (i, j) in &overlaps {
            println!("  Overlap between:");
            println!("    Chunk {}: lines {}-{} ({})", i, chunks[*i].start_line, chunks[*i].end_line, chunks[*i].element_type);
            println!("    Chunk {}: lines {}-{} ({})", j, chunks[*j].start_line, chunks[*j].end_line, chunks[*j].element_type);
        }
    }
    
    assert!(overlaps.is_empty(), "Rust parser should not create overlapping chunks");
}

#[test]
fn test_no_overlaps_in_python_parser() {
    let test_content = r#"
# Test file for overlap detection
import os
from typing import List

class Point:
    """A simple point class"""
    
    def __init__(self, x: int, y: int):
        self.x = x
        self.y = y
    
    def distance(self) -> float:
        """Calculate distance from origin"""
        return (self.x ** 2 + self.y ** 2) ** 0.5

def main():
    """Main function"""
    p = Point(3, 4)
    print(f"Distance: {p.distance()}")

def helper():
    """Helper function"""
    print("Helper")

# Nested classes
class Outer:
    class Inner:
        def method(self):
            pass
"#;
    
    let temp_file = "/tmp/test_overlap_python.py";
    fs::write(temp_file, test_content).unwrap();
    
    let chunks = get_chunks(Path::new(temp_file)).unwrap();
    let overlaps = check_overlaps(&chunks);
    
    if !overlaps.is_empty() {
        println!("Found {} overlaps in Python parser:", overlaps.len());
        for (i, j) in &overlaps {
            println!("  Overlap between:");
            println!("    Chunk {}: lines {}-{} ({})", i, chunks[*i].start_line, chunks[*i].end_line, chunks[*i].element_type);
            println!("    Chunk {}: lines {}-{} ({})", j, chunks[*j].start_line, chunks[*j].end_line, chunks[*j].element_type);
        }
    }
    
    assert!(overlaps.is_empty(), "Python parser should not create overlapping chunks");
}

#[test]
fn test_no_overlaps_in_javascript_parser() {
    let test_content = r#"
// Test file for overlap detection
import { something } from 'module';

class Point {
    constructor(x, y) {
        this.x = x;
        this.y = y;
    }
    
    distance() {
        return Math.sqrt(this.x ** 2 + this.y ** 2);
    }
}

function main() {
    const p = new Point(3, 4);
    console.log(`Distance: ${p.distance()}`);
}

const helper = () => {
    console.log("Helper");
};

// Nested functions
function outer() {
    function inner() {
        return 42;
    }
    return inner();
}
"#;
    
    let temp_file = "/tmp/test_overlap_js.js";
    fs::write(temp_file, test_content).unwrap();
    
    let chunks = get_chunks(Path::new(temp_file)).unwrap();
    let overlaps = check_overlaps(&chunks);
    
    if !overlaps.is_empty() {
        println!("Found {} overlaps in JavaScript parser:", overlaps.len());
        for (i, j) in &overlaps {
            println!("  Overlap between:");
            println!("    Chunk {}: lines {}-{} ({})", i, chunks[*i].start_line, chunks[*i].end_line, chunks[*i].element_type);
            println!("    Chunk {}: lines {}-{} ({})", j, chunks[*j].start_line, chunks[*j].end_line, chunks[*j].element_type);
        }
    }
    
    assert!(overlaps.is_empty(), "JavaScript parser should not create overlapping chunks");
}

#[test]
fn test_no_overlaps_in_go_parser() {
    let test_content = r#"
package main

import (
    "fmt"
    "math"
)

type Point struct {
    X int
    Y int
}

func (p Point) Distance() float64 {
    return math.Sqrt(float64(p.X*p.X + p.Y*p.Y))
}

func main() {
    p := Point{X: 3, Y: 4}
    fmt.Printf("Distance: %f\n", p.Distance())
}

func helper() {
    fmt.Println("Helper")
}
"#;
    
    let temp_file = "/tmp/test_overlap_go.go";
    fs::write(temp_file, test_content).unwrap();
    
    let chunks = get_chunks(Path::new(temp_file)).unwrap();
    let overlaps = check_overlaps(&chunks);
    
    if !overlaps.is_empty() {
        println!("Found {} overlaps in Go parser:", overlaps.len());
        for (i, j) in &overlaps {
            println!("  Overlap between:");
            println!("    Chunk {}: lines {}-{} ({})", i, chunks[*i].start_line, chunks[*i].end_line, chunks[*i].element_type);
            println!("    Chunk {}: lines {}-{} ({})", j, chunks[*j].start_line, chunks[*j].end_line, chunks[*j].element_type);
        }
    }
    
    assert!(overlaps.is_empty(), "Go parser should not create overlapping chunks");
}

#[test]
fn test_no_overlaps_in_typescript_parser() {
    let test_content = r#"
// Test file for overlap detection
import { Something } from './module';

interface IPoint {
    x: number;
    y: number;
}

class Point implements IPoint {
    constructor(public x: number, public y: number) {}
    
    distance(): number {
        return Math.sqrt(this.x ** 2 + this.y ** 2);
    }
}

function main(): void {
    const p = new Point(3, 4);
    console.log(`Distance: ${p.distance()}`);
}

const helper = (): void => {
    console.log("Helper");
};

type PointTuple = [number, number];
"#;
    
    let temp_file = "/tmp/test_overlap_ts.ts";
    fs::write(temp_file, test_content).unwrap();
    
    let chunks = get_chunks(Path::new(temp_file)).unwrap();
    let overlaps = check_overlaps(&chunks);
    
    if !overlaps.is_empty() {
        println!("Found {} overlaps in TypeScript parser:", overlaps.len());
        for (i, j) in &overlaps {
            println!("  Overlap between:");
            println!("    Chunk {}: lines {}-{} ({})", i, chunks[*i].start_line, chunks[*i].end_line, chunks[*i].element_type);
            println!("    Chunk {}: lines {}-{} ({})", j, chunks[*j].start_line, chunks[*j].end_line, chunks[*j].element_type);
        }
    }
    
    assert!(overlaps.is_empty(), "TypeScript parser should not create overlapping chunks");
}