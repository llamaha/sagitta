#!/usr/bin/env python3
"""
Script to analyze code parser outputs and detect overlapping chunks.
This will help identify if any language parsers are creating overlapping chunks.
"""

import json
import subprocess
import sys
from pathlib import Path
from typing import List, Dict, Tuple

def get_chunks_for_file(file_path: str) -> List[Dict]:
    """Call sagitta-cli to get chunks for a file."""
    # Use a simple rust file for testing
    test_content = '''
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
'''
    
    # Write test file
    test_path = Path("/tmp/test_overlap.rs")
    test_path.write_text(test_content)
    
    # Parse using code-parsers crate directly
    # We'll need to use the rust code to parse this
    return []

def check_overlaps(chunks: List[Dict]) -> List[Tuple[Dict, Dict]]:
    """Check for overlapping chunks."""
    overlaps = []
    
    for i, chunk1 in enumerate(chunks):
        for j, chunk2 in enumerate(chunks[i+1:], i+1):
            # Check if chunks overlap
            if chunk1['file_path'] == chunk2['file_path']:
                # Check line overlap
                if (chunk1['start_line'] <= chunk2['end_line'] and 
                    chunk2['start_line'] <= chunk1['end_line']):
                    overlaps.append((chunk1, chunk2))
    
    return overlaps

def main():
    """Main function to test overlap detection."""
    print("Testing for chunk overlaps in parsers...")
    
    # Test different file types
    test_files = [
        ("test.rs", "rust"),
        ("test.py", "python"), 
        ("test.js", "javascript"),
        ("test.go", "go"),
        ("test.ts", "typescript"),
    ]
    
    for filename, lang in test_files:
        print(f"\nTesting {lang} parser with {filename}...")
        chunks = get_chunks_for_file(filename)
        if chunks:
            overlaps = check_overlaps(chunks)
            if overlaps:
                print(f"FOUND {len(overlaps)} OVERLAPS in {lang}!")
                for c1, c2 in overlaps:
                    print(f"  Chunk 1: lines {c1['start_line']}-{c1['end_line']} ({c1['element_type']})")
                    print(f"  Chunk 2: lines {c2['start_line']}-{c2['end_line']} ({c2['element_type']})")
            else:
                print(f"No overlaps found in {lang}")

if __name__ == "__main__":
    main()