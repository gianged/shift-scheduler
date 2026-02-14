### Note:

## Performance Consideration

- HTTP client responses use owned deserialization (`String` / `DeserializeOwned`).
  For high-throughput scenarios involving large payloads, zero-copy deserialization
  (`&str` / `Deserialize<'de>`) could reduce heap allocations by borrowing directly
  from the response buffer. This was not implemented as the current data volume
  (tens of staff per request) does not warrant the added lifetime complexity.
