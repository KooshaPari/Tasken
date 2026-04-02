---
layout: doc
title: Hello World Story
---

# Hello World: Your First Tasken Operation

<StoryHeader
    title="First Operation"
    duration="2"
    difficulty="beginner"
    :gif="'/gifs/tasken-hello-world.gif'"
/>

## Objective

Execute your first Tasken operation successfully.

## Prerequisites

- Rust/Node/Python installed
- Tasken CLI installed

## Implementation

```rust
use tasken::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new().await?;
    let result = client.hello().await?;
    println!("Success: {}", result);
    Ok(())
}
```

## Expected Output

```
Success: Hello from Tasken!
```

## Next Steps

- [Core Integration](./core-integration)
- [API Reference](../reference/api)
