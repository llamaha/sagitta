#!/bin/bash

# Benchmark script for sagitta-cli
# Dependencies: yq (for YAML parsing), jq (for JSON parsing)
# On Debian/Ubuntu: sudo apt-get install yq jq
# On macOS: brew install yq jq

echo "Starting benchmark script..."

SCRIPT_DIR=$(dirname "$0") # Get the directory where the script resides
CONFIG_FILE="$SCRIPT_DIR/benchmark_config.yaml"
REPORT_FILE="$SCRIPT_DIR/benchmark_results.md"

echo "Script directory: $SCRIPT_DIR"
echo "Config file path: $CONFIG_FILE"
echo "Report file path: $REPORT_FILE"

# --- Prerequisites Check ---
echo "Checking for yq..."
if ! command -v yq &> /dev/null; then
    echo "Error: yq is not installed or not in PATH. Please install yq."
    echo "On Debian/Ubuntu: sudo apt-get install yq"
    echo "On macOS: brew install yq"
    exit 1
fi
echo "yq found."

echo "Checking for jq..."
if ! command -v jq &> /dev/null; then
    echo "Error: jq is not installed or not in PATH. Please install jq."
    echo "On Debian/Ubuntu: sudo apt-get install jq"
    echo "On macOS: brew install jq"
    exit 1
fi
echo "jq found."
# --- End Prerequisites Check ---

echo "Ensuring config file exists at $CONFIG_FILE..."
# Ensure config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Config file not found at $CONFIG_FILE"
    exit 1
fi
echo "Config file found."

echo "Reading global settings from config..."
CLI_PATH=$(yq e '.global_settings.cli_path' "$CONFIG_FILE" 2>/dev/null)
RESULTS_LIMIT=$(yq e '.global_settings.results_limit' "$CONFIG_FILE" 2>/dev/null)
echo "CLI_PATH set to: '$CLI_PATH'"
echo "RESULTS_LIMIT set to: '$RESULTS_LIMIT'"

# Check if CLI_PATH is set and valid
if [ -z "$CLI_PATH" ]; then
    echo "Error: cli_path not set in $CONFIG_FILE under global_settings.cli_path"
    exit 1
fi

echo "Checking if CLI_PATH '$CLI_PATH' is executable or in PATH..."
# Check if CLI_PATH is executable or in PATH
if ! command -v "$CLI_PATH" &> /dev/null && [ ! -x "$CLI_PATH" ]; then
    echo "Error: sagitta-cli not found at '$CLI_PATH' or not in PATH, or not executable. Please check benchmark_config.yaml."
    exit 1
fi
echo "CLI_PATH is valid and accessible."

echo "Initializing or clearing report file: $REPORT_FILE"
# Initialize or clear the report file
echo "# Sagitta Query Benchmark Results" > "$REPORT_FILE"
echo "Generated on $(date)" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "Instructions: For each query, review the N results provided by sagitta-cli." >> "$REPORT_FILE"
echo "Rate the relevance of each *individual* result on a scale of 1 (not relevant) to 10 (highly relevant)." >> "$REPORT_FILE"
echo "The goal is to assess the quality of search results to identify areas for improvement." >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

# Read repositories from the config file
repo_count=$(yq e '.repositories | length' "$CONFIG_FILE" 2>/dev/null)

echo "Found $repo_count repositories in config."

for i in $(seq 0 $(($repo_count - 1))); do
    repo_name=$(yq e ".repositories[$i].name" "$CONFIG_FILE" 2>/dev/null)
    repo_url=$(yq e ".repositories[$i].url" "$CONFIG_FILE" 2>/dev/null)
    repo_lang_display=$(yq e ".repositories[$i].language" "$CONFIG_FILE" 2>/dev/null)
    repo_branch_to_use=$(yq e ".repositories[$i].default_branch // \"\"" "$CONFIG_FILE" 2>/dev/null) # Added for branch selection

    echo "" | tee -a "$REPORT_FILE"
    echo "## Processing Repository: $repo_name (URL: $repo_url, Language: $repo_lang_display)" | tee -a "$REPORT_FILE"
    if [ -n "$repo_branch_to_use" ]; then # Added for branch display
        echo "(Attempting to use branch: $repo_branch_to_use)" | tee -a "$REPORT_FILE"
    fi
    echo "-----------------------------------------------------------------" | tee -a "$REPORT_FILE"
    echo "" | tee -a "$REPORT_FILE"


    echo "Attempting to clear repository $repo_name (if it exists)..."
    # RAYON_NUM_THREADS might not be relevant for 'clear', but harmless
    RAYON_NUM_THREADS=4 "$CLI_PATH" repo clear --name "$repo_name" -y || echo "Clear failed or repo '$repo_name' did not exist, continuing..."

    echo "Adding repository $repo_name from $repo_url..."
    add_cmd_array=("RAYON_NUM_THREADS=4" "$CLI_PATH" repo add --name "$repo_name" --url "$repo_url")
    if [ -n "$repo_branch_to_use" ]; then # Added branch logic
        add_cmd_array+=("--branch" "$repo_branch_to_use")
    fi
    echo "Executing add command: ${add_cmd_array[*]}"

    if ! env RAYON_NUM_THREADS=4 "${add_cmd_array[@]:1}"; then
        echo "Failed to add repository $repo_name. Skipping." | tee -a "$REPORT_FILE"
        echo "" >> "$REPORT_FILE"
        continue
    fi

    echo "Syncing repository $repo_name..."
    if ! env RAYON_NUM_THREADS=4 "$CLI_PATH" repo sync "$repo_name"; then # Added RAYON_NUM_THREADS
        echo "Failed to sync repository $repo_name. Skipping queries for this repo." | tee -a "$REPORT_FILE"
        echo "" >> "$REPORT_FILE"
        continue
    fi
    echo "Sync complete for $repo_name."

    query_count=$(yq e ".repositories[$i].queries | length" "$CONFIG_FILE" 2>/dev/null)
    echo "Found $query_count queries for $repo_name."

    if [ "$query_count" -gt 0 ]; then # Check if there are any queries defined
        # Select one random query index
        j=$(($RANDOM % $query_count))
        echo "Randomly selected query index $j for $repo_name."

        query_text=$(yq e ".repositories[$i].queries[$j].text" "$CONFIG_FILE" 2>/dev/null)
        query_lang_opt=$(yq e ".repositories[$i].queries[$j].lang // \"\"" "$CONFIG_FILE" 2>/dev/null)
        query_type_opt=$(yq e ".repositories[$i].queries[$j].type // \"\"" "$CONFIG_FILE" 2>/dev/null)

        # Using an array for command construction for safety
        cmd_array=("$CLI_PATH" repo query --limit "$RESULTS_LIMIT" --name "$repo_name" --json "$query_text")

        if [ -n "$query_lang_opt" ]; then
            cmd_array+=("--lang" "$query_lang_opt")
        fi
        if [ -n "$query_type_opt" ]; then
            cmd_array+=("--type" "$query_type_opt")
        fi

        echo "" >> "$REPORT_FILE"
        # For display, quote arguments that might have spaces
        query_cmd_display_array=()
        for arg in "${cmd_array[@]}"; do
            if [[ "$arg" == *\ * ]]; then # Simple check for spaces
                query_cmd_display_array+=("\"$arg\"")
            else
                query_cmd_display_array+=("$arg")
            fi
        done
        echo "### Query: \`${query_cmd_display_array[*]}\`" >> "$REPORT_FILE"
        echo "" >> "$REPORT_FILE"
        echo "**Results (Rate 1-10 for relevance):**" >> "$REPORT_FILE"
        echo "" >> "$REPORT_FILE"

        echo "Executing: ${cmd_array[*]}"
        # Capture raw output first
        raw_cli_output=$("${cmd_array[@]}")
        exit_code=$?

        # Attempt to extract JSON part from raw_cli_output
        # This awk command prints from the first line that starts with '{' to the end.
        query_output=$(echo "$raw_cli_output" | awk '/^{/{p=1} p')

        if [ $exit_code -ne 0 ]; then
            echo "Error executing query. Exit code: $exit_code" >> "$REPORT_FILE"
            echo "Command: ${cmd_array[*]}" >> "$REPORT_FILE"
            echo "\\\`\\\`\\\`" >> "$REPORT_FILE"
            echo "$raw_cli_output" >> "$REPORT_FILE" # Log the original raw output
            echo "\\\`\\\`\\\`" >> "$REPORT_FILE"
        # Check if query_output (potentially extracted JSON) is valid
        elif [ -n "$query_output" ] && jq -e . >/dev/null 2>&1 <<<"$query_output"; then
            num_results=$(jq '.results | length' <<< "$query_output")
            if [ -z "$num_results" ] || [ "$num_results" -eq 0 ]; then
                 echo "No results returned or error in parsing JSON output (parsed as empty)." >> "$REPORT_FILE"
                 if [ -n "$query_output" ]; then
                    echo "Raw output:" >> "$REPORT_FILE"
                    echo "\\\`\\\`\\\`" >> "$REPORT_FILE"
                    echo "$query_output" >> "$REPORT_FILE"
                    echo "\\\`\\\`\\\`" >> "$REPORT_FILE"
                 fi
            else
                for k in $(seq 0 $(($num_results - 1))); do
                    result_snippet=$(jq -r ".results[$k].content" <<< "$query_output")
                    result_file=$(jq -r ".results[$k].file_path" <<< "$query_output")
                    result_score=$(jq -r ".results[$k].score" <<< "$query_output")

                    result_snippet=${result_snippet:-Snippet not found}
                    result_file=${result_file:-File path not found}
                    result_score=${result_score:-N/A}

                    echo "**Result $(($k + 1)):** (Score: $result_score)" >> "$REPORT_FILE"
                    echo "\\\`\\\`\\\`" >> "$REPORT_FILE"
                    # Sanitize snippet for markdown: escape backticks using here-string for sed
                    result_snippet_md=$(sed 's/`/\\\\`/g' <<< "$result_snippet")
                    echo "$result_snippet_md" >> "$REPORT_FILE"
                    echo "\\\`\\\`\\\`" >> "$REPORT_FILE"
                    echo "**File:** \\\`$result_file\\\`" >> "$REPORT_FILE"
                    echo "**Rating (1-10):** ______" >> "$REPORT_FILE"
                    echo "" >> "$REPORT_FILE"
                done
            fi
        else # Handles cases where exit_code was 0 but JSON extraction failed or JSON is invalid
            echo "Raw output from CLI (JSON parsing failed or JSON not found/extracted):" >> "$REPORT_FILE"
            echo "Command: ${cmd_array[*]}" >> "$REPORT_FILE"
            echo "\\\`\\\`\\\`" >> "$REPORT_FILE"
            # Sanitize original raw_cli_output for markdown display using here-string for sed
            raw_cli_output_md=$(sed 's/`/\\\\`/g' <<< "$raw_cli_output")
            echo "$raw_cli_output_md" >> "$REPORT_FILE"
            echo "\\\`\\\`\\\`" >> "$REPORT_FILE"
            echo "_Manual formatting/rating needed for the above raw output._" >> "$REPORT_FILE"
            echo "**Rating (1-10) for overall raw output quality:** ______" >> "$REPORT_FILE"
        fi
        echo "" >> "$REPORT_FILE"
        echo "---" >> "$REPORT_FILE"
    else
        echo "No queries defined for $repo_name. Skipping query execution."
    fi
    echo "" >> "$REPORT_FILE"

done

echo "Benchmark completed. Report generated at $REPORT_FILE" 