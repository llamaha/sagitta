# Qdrant Database Wipe Recovery

## The Problem

When Qdrant database is wiped/reset but the Sagitta config still shows repositories as synced:
1. Config has `last_synced_commits` entries showing repos are up-to-date
2. Git logic sees no changes, skips sync
3. But Qdrant collections don't exist!
4. Users get "Collection does not exist" errors when querying

## The Solution

### Automatic Recovery (Implemented)

The sync process now validates collections before skipping:

1. **Before skipping sync**: Checks if collection exists in Qdrant
2. **If missing**: Forces a full sync automatically
3. **If empty**: Also forces a full sync
4. **Graceful handling**: Clear log messages explain what's happening

### Manual Recovery Options

Users have several ways to recover:

#### Option 1: Force Sync (Easiest)
```bash
sagitta-cli repo sync --name <repo> --force
```

#### Option 2: Clear and Re-sync
```bash
sagitta-cli repo clear --name <repo>
sagitta-cli repo sync --name <repo>
```

#### Option 3: Use New Repair Command (Future)
```bash
sagitta-cli repo repair --name <repo>
# or repair all repos
sagitta-cli repo repair --all
```

## Implementation Details

### Collection Validation

Before deciding to skip sync, the system now:

```rust
// Check if collection exists
let collection_exists = client.collection_exists(collection_name).await?;

if !collection_exists {
    warn!("Collection does not exist but config shows synced. Forcing full sync.");
    sync_needed = true;
} else {
    // Check if collection has content
    let info = client.get_collection_info(collection_name).await?;
    if info.points_count.unwrap_or(0) == 0 {
        warn!("Collection exists but is empty. Forcing full sync.");
        sync_needed = true;
    }
}
```

### Test Coverage

Added comprehensive tests for:
- Qdrant completely wiped (collections don't exist)
- Collections exist but are empty
- Force sync always re-indexes
- Concurrent operations handle missing collections

## Benefits

1. **Self-healing**: System automatically recovers from Qdrant wipes
2. **Clear feedback**: Users see why full sync is happening
3. **No manual intervention**: Works transparently
4. **Prevents confusion**: No more "collection doesn't exist" errors

## Future Enhancements

### 1. Repair Command
```bash
sagitta-cli repo repair [--name <repo>] [--all]
```
Would:
- Validate all collections
- Clear sync metadata for missing collections
- Optionally trigger re-sync

### 2. Health Check Command
```bash
sagitta-cli repo health [--name <repo>]
```
Would report:
- Collection existence
- Point counts
- Sync status mismatches
- Suggested fixes

### 3. Persistent Metadata
Store sync metadata in Qdrant collection metadata:
- Last sync timestamp
- Commit hash
- File count
This would survive config file issues.

## Edge Cases Covered

1. **Qdrant completely wiped**: ✅ Auto-recovers
2. **Partial wipe** (some collections missing): ✅ Recovers affected repos
3. **Empty collections**: ✅ Detects and re-syncs
4. **Network failures during check**: ✅ Falls back to full sync
5. **Concurrent syncs**: ✅ Each validates independently

## User Experience

Before:
```
$ sagitta-cli repo query "search term"
Error: Collection 'repo_myproject_br_abc123' does not exist

$ sagitta-cli repo sync --name myproject  
Repository already synced to commit abc123
```

After:
```
$ sagitta-cli repo sync --name myproject
[WARN] Collection 'repo_myproject_br_abc123' does not exist but config shows repository was synced. Forcing full sync.
[INFO] Gathering all files for full sync due to missing/empty collection...
[INFO] Full sync: 1523 files found in tree.
[INFO] Successfully synced repository 'myproject'
```

The system now "just works" even after catastrophic Qdrant failures!