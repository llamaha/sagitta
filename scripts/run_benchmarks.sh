#!/bin/bash
# Run ONNX benchmarks with different configurations and compare results

# Set the output directory
OUTDIR="benchmark_results"
mkdir -p $OUTDIR

# Get the current timestamp
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

# Set the model paths (adjust these as needed)
MODEL_PATH="onnx/all-minilm-l12-v2.onnx"
TOKENIZER_PATH="onnx/minilm_tokenizer"

# Create a samples file if it doesn't exist
SAMPLES_FILE="$OUTDIR/samples.txt"
if [ ! -f "$SAMPLES_FILE" ]; then
    echo "Creating sample texts file..."
    cat > "$SAMPLES_FILE" << EOF
This is a short sample text for benchmarking
Another short example text
Rust is a modern programming language
ONNX Runtime provides efficient inference
This is a longer example that contains multiple sentences for testing the effects of sequence length on the batch processing logic. It's important to have a variety of text lengths.
The embedding model should be able to handle different input lengths efficiently. The batch processor needs to group inputs with similar sequence lengths to maximize throughput.
Session pooling and warmup are techniques to improve cold-start performance by maintaining a pool of initialized inference sessions and warming them up.
EOF
    echo "Created samples file at $SAMPLES_FILE"
fi

# Function to run a benchmark and save results
run_benchmark() {
    local name=$1
    local provider=$2
    local batch_sizes=$3
    local pre_warm=$4
    local dynamic_batching=$5
    local output_file="$OUTDIR/${TIMESTAMP}_${name}.csv"
    
    echo "Running benchmark: $name"
    echo "Provider: $provider, Pre-warm: $pre_warm, Dynamic batching: $dynamic_batching"
    
    cargo run --release --bin onnx_benchmark -- \
        --model-path "$MODEL_PATH" \
        --tokenizer-path "$TOKENIZER_PATH" \
        --provider "$provider" \
        --batch-sizes "$batch_sizes" \
        --pre-warm "$pre_warm" \
        --dynamic-batching "$dynamic_batching" \
        --samples-file "$SAMPLES_FILE" \
        --csv > "$output_file"
    
    echo "Results saved to $output_file"
    echo
}

# Baseline benchmark
echo "=== Running Baseline (Basic Provider) ==="
run_benchmark "baseline" "basic" "1,4,8,16,32" "false" "false"

# Phase 2 optimizations
echo "=== Running Phase 2 Optimizations ==="
run_benchmark "phase2" "optimized" "1,4,8,16,32" "false" "false"

# Phase 3 optimizations - Session Warmup
echo "=== Running Phase 3 (Session Warmup) ==="
run_benchmark "phase3_warmup" "optimized" "1,4,8,16,32" "true" "false"

# Phase 3 optimizations - Dynamic Batching
echo "=== Running Phase 3 (Dynamic Batching) ==="
run_benchmark "phase3_dynbatch" "optimized" "1,4,8,16,32" "false" "true"

# Phase 3 optimizations - All
echo "=== Running Phase 3 (All Optimizations) ==="
run_benchmark "phase3_all" "optimized" "1,4,8,16,32" "true" "true"

# Compare results
echo "=== Benchmark Comparison ==="
echo "Results are saved in the $OUTDIR directory with timestamp $TIMESTAMP"
echo "You can compare them with:"
echo "  cat $OUTDIR/${TIMESTAMP}_*.csv | sort -t, -k2,2n -k1,1"

# Generate a simple comparison if gnuplot is available
if command -v gnuplot &> /dev/null; then
    PLOT_FILE="$OUTDIR/${TIMESTAMP}_comparison.png"
    echo "Generating comparison plot..."
    
    # Create gnuplot script
    GNUPLOT_SCRIPT="$OUTDIR/plot.gnu"
    cat > "$GNUPLOT_SCRIPT" << EOF
set terminal png size 1200,800
set output "$PLOT_FILE"
set title "ONNX Embedding Performance Comparison"
set xlabel "Batch Size"
set ylabel "Throughput (samples/sec)"
set grid
set key outside right
set style data linespoints
set style line 1 lt 1 lw 2 pt 7 ps 1.5 lc rgb "red"
set style line 2 lt 1 lw 2 pt 9 ps 1.5 lc rgb "blue"
set style line 3 lt 1 lw 2 pt 5 ps 1.5 lc rgb "green"
set style line 4 lt 1 lw 2 pt 11 ps 1.5 lc rgb "purple"
set style line 5 lt 1 lw 2 pt 13 ps 1.5 lc rgb "orange"

plot "$OUTDIR/${TIMESTAMP}_baseline.csv" using 2:6 title "Baseline" with linespoints ls 1, \\
     "$OUTDIR/${TIMESTAMP}_phase2.csv" using 2:6 title "Phase 2" with linespoints ls 2, \\
     "$OUTDIR/${TIMESTAMP}_phase3_warmup.csv" using 2:6 title "Phase 3 (Warmup)" with linespoints ls 3, \\
     "$OUTDIR/${TIMESTAMP}_phase3_dynbatch.csv" using 2:6 title "Phase 3 (Dynamic Batching)" with linespoints ls 4, \\
     "$OUTDIR/${TIMESTAMP}_phase3_all.csv" using 2:6 title "Phase 3 (All)" with linespoints ls 5
EOF
    
    gnuplot "$GNUPLOT_SCRIPT"
    echo "Comparison plot saved to $PLOT_FILE"
fi

echo "All benchmarks completed!" 