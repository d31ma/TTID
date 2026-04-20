# TTID (Time-Tagged Identifier)

A lightweight, time-based identifier generator that tracks creation, update, and deletion timestamps using a progressive format.

## Overview

TTID creates unique identifiers with a progressive structure:
- **Created:** `[CREATION_TIMESTAMP]`
- **Updated:** `[CREATION_TIMESTAMP]-[UPDATE_TIMESTAMP]`
- **Deleted:** `[CREATION_TIMESTAMP]-[UPDATE_TIMESTAMP]-[DELETION_TIMESTAMP]`

Each TTID segment contains:
- High-resolution timestamps encoded in base-36
- Progressive expansion to track lifecycle states
- Compact 11-character timestamps for efficiency
- Immutable deletion state (cannot be modified once deleted)

## Installation

Authenticate with GitHub Packages before installing:

```bash
npm login --scope=@d31ma --auth-type=legacy --registry=https://npm.pkg.github.com
```

Then add this to your user or project `.npmrc`:

```ini
@d31ma:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=YOUR_GITHUB_PAT
```

See GitHub's npm registry docs for the latest authentication details:
https://docs.github.com/packages/using-github-packages-with-your-projects-ecosystem/configuring-npm-for-use-with-github-packages

```bash
npm install @d31ma/ttid
```

## Usage

### Basic ID Generation

```typescript
import TTID from '@d31ma/ttid';

// Generate a new TTID (creation only)
const newId = TTID.generate();
console.log(newId);
// Example output: "1A2B3C4D5E6"

// Verify if a string is a valid TTID
const isValid = TTID.isTTID(newId);
console.log(isValid); // Returns Date object if valid, null if invalid
```

### Progressive Updates

```typescript
import TTID from '@d31ma/ttid';

// Start with a new ID
let id = TTID.generate();
console.log(id); // "1A2B3C4D5E6"

// Update the ID (adds update timestamp)
id = TTID.generate(id);
console.log(id); // "1A2B3C4D5E6-F7G8H9I0J1"

// Update again (replaces update timestamp)
id = TTID.generate(id);
console.log(id); // "1A2B3C4D5E6-K2L3M4N5O6"

// Mark as deleted (adds deletion timestamp - final state)
id = TTID.generate(id, true);
console.log(id); // "1A2B3C4D5E6-K2L3M4N5O6-P7Q8R9S0T1"

// Attempting to modify a deleted ID throws an error
try {
  TTID.generate(id); // Throws: "This identifier can no longer be modified"
} catch (error) {
  console.error(error.message);
}
```

### Decoding Timestamps

```typescript
import TTID from '@d31ma/ttid';

// Create and update an ID
let id = TTID.generate();
setTimeout(() => {
  id = TTID.generate(id);
  
  setTimeout(() => {
    id = TTID.generate(id, true); // Mark as deleted
    
    // Decode all timestamps
    const times = TTID.decodeTime(id);
    console.log(times);
    // Example output:
    // {
    //   createdAt: 1651234567890,
    //   updatedAt: 1651234572345,
    //   deletedAt: 1651234578901
    // }
    
    // Convert to readable dates
    console.log({
      created: new Date(times.createdAt),
      updated: times.updatedAt ? new Date(times.updatedAt) : null,
      deleted: times.deletedAt ? new Date(times.deletedAt) : null
    });
    
  }, 2000);
}, 1000);
```

### Working with Different States

```typescript
import TTID from '@d31ma/ttid';

// Check ID states
function analyzeId(id: string) {
  const validation = TTID.isTTID(id);
  
  if (!validation) {
    console.log('Invalid TTID');
    return;
  }
  
  const segments = id.split('-');
  const times = TTID.decodeTime(id);
  
  console.log(`ID State: ${getIdState(segments.length)}`);
  console.log(`Created: ${new Date(times.createdAt)}`);
  
  if (times.updatedAt) {
    console.log(`Updated: ${new Date(times.updatedAt)}`);
  }
  
  if (times.deletedAt) {
    console.log(`Deleted: ${new Date(times.deletedAt)}`);
  }
}

function getIdState(segmentCount: number) {
  switch (segmentCount) {
    case 1: return 'Created';
    case 2: return 'Updated';
    case 3: return 'Deleted';
    default: return 'Unknown';
  }
}

// Examples
const createdId = TTID.generate();
analyzeId(createdId); // ID State: Created

const updatedId = TTID.generate(createdId);
analyzeId(updatedId); // ID State: Updated

const deletedId = TTID.generate(updatedId, true);
analyzeId(deletedId); // ID State: Deleted
```

### Error Handling

```typescript
import TTID from '@d31ma/ttid';

// Invalid ID format
try {
  TTID.generate('invalid-id');
} catch (error) {
  console.error(error.message); // "Invalid TTID!"
}

// Attempting to modify deleted ID
const id = TTID.generate();
const updated = TTID.generate(id);
const deleted = TTID.generate(updated, true);

try {
  TTID.generate(deleted);
} catch (error) {
  console.error(error.message); // "This identifier can no longer be modified"
}

// Invalid format for decoding
try {
  TTID.decodeTime('not-a-ttid');
} catch (error) {
  console.error(error.message); // "Invalid Format!"
}
```

## API Reference

### `TTID.generate(id?: string, del?: boolean)`

Generates a new TTID or updates an existing one.

**Parameters:**
- `id` (optional) - An existing TTID to update
- `del` (optional) - Set to `true` to mark the ID as deleted

**Returns:** `_ttid` - A TTID string

**Behavior:**
- No parameters: Creates new ID `[TIMESTAMP]`
- Valid TTID provided: Updates to `[CREATED]-[NEW_TIMESTAMP]`
- Valid TTID + `del=true`: Marks as deleted `[CREATED]-[UPDATED]-[DELETED_TIMESTAMP]`

**Throws:** 
- Error if provided ID is invalid
- Error if attempting to modify a deleted ID (3 segments)

### `TTID.decodeTime(id: string)`

Decodes timestamps from a TTID.

**Parameters:**
- `id` - A TTID string

**Returns:** `_timestamps` object with:
- `createdAt` - Creation timestamp in milliseconds
- `updatedAt` (optional) - Update timestamp in milliseconds
- `deletedAt` (optional) - Deletion timestamp in milliseconds

**Throws:** Error if the format is invalid

### `TTID.isTTID(id: string)`

Validates a TTID and returns creation date if valid.

**Parameters:**
- `id` - A string to validate

**Returns:** 
- `Date` object (creation date) if valid
- `null` if invalid

### `TTID.isUUID(id: string)`

Checks if a string is a valid UUID.

**Parameters:**
- `id` - A string to check

**Returns:** `RegExpMatchArray | null` - Match result or null

## Format Specification

TTIDs follow a strict format:
- Base-36 encoding (0-9, A-Z)
- 11-character timestamps
- Hyphen-separated segments
- Progressive structure

**Valid Patterns:**
- `[A-Z0-9]{11}` - Created only
- `[A-Z0-9]{11}-[A-Z0-9]{1,11}` - Created + Updated
- `[A-Z0-9]{11}-[A-Z0-9]{1,11}-[A-Z0-9]{1,11}` - Created + Updated + Deleted

**Special Cases:**
- Placeholder 'X' may appear in update position for certain states
- Deleted IDs cannot be modified further

## Lifecycle States

| State | Format | Segments | Modifiable |
|-------|--------|----------|------------|
| Created | `TIMESTAMP` | 1 | ✅ |
| Updated | `CREATED-UPDATED` | 2 | ✅ |
| Deleted | `CREATED-UPDATED-DELETED` | 3 | ❌ |

## Comparison with Other Systems

| Feature | TTID | UUID | ULID | 
|---------|------|------|------|
| Progressive states | ✅ | ❌ | ❌ |
| Soft delete tracking | ✅ | ❌ | ❌ |
| Immutable final state | ✅ | ❌ | ❌ |
| Compact encoding | ✅ | ❌ | ✅ |
| Time-based | ✅ | ⚠️ | ✅ |
| Fixed length | ❌ | ✅ | ✅ |

## Use Cases

- **Database Records**: Track entity lifecycle (created → updated → soft deleted)
- **Audit Systems**: Maintain chronological history in the ID itself
- **Document Management**: Version control with embedded timestamps
- **API Resources**: RESTful endpoints with state-aware identifiers
- **Event Sourcing**: Compact event identifiers with temporal information

## Performance Considerations

- Base-36 encoding provides compact representation
- Progressive format minimizes storage for simple states
- High-resolution timestamps ensure uniqueness in high-frequency scenarios
- Validation includes timestamp parsing for integrity checking

## Security

`_ttid` is a TypeScript template-literal type, not a runtime-enforced brand. TypeScript alone cannot prevent a plain `string` from being used where a `_ttid` is expected.

**Rule:** always obtain TTID values via `TTID.generate()` or validate them with `TTID.isTTID()` before using them in any security-sensitive context (database keys, access-control checks, audit logs).

```typescript
const raw: string = externalInput()
const valid = TTID.isTTID(raw)   // returns Date | null
if (!valid) throw new Error('Invalid identifier')
// safe to use raw as _ttid from here
```

Input length is bounded to 36 characters before any regex evaluation, preventing CPU exhaustion from pathological inputs.

## License

MIT
