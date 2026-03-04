# Cross-Language Examples

Examples using MoFA from different programming languages.

## Python

**Location:** `examples/python_bindings/`

```python
import os
from mofa import LLMClient, AgentInput

os.environ["OPENAI_API_KEY"] = "sk-..."

client = LLMClient.from_env()
response = client.ask("What is Rust?")
print(response)
```

## Java

**Location:** `examples/java_bindings/`

```java
import org.mofa.sdk.*;

public class Main {
    public static void main(String[] args) {
        System.setProperty("OPENAI_API_KEY", "sk-...");

        LLMClient client = LLMClient.fromEnv();
        String response = client.ask("What is Rust?");
        System.out.println(response);
    }
}
```

## Go

**Location:** `examples/go_bindings/`

```go
package main

import (
    "os"
    "fmt"
    "github.com/mofa-org/mofa-go/mofa"
)

func main() {
    os.Setenv("OPENAI_API_KEY", "sk-...")

    client := mofa.NewLLMClient()
    response := client.Ask("What is Rust?")
    fmt.Println(response)
}
```

## Running Examples

```bash
# Python
cd examples/python_bindings
pip install -r requirements.txt
python main.py

# Java
cd examples/java_bindings
./gradlew run

# Go
cd examples/go_bindings
go run main.go
```

## See Also

- [Cross-Language Bindings](../ffi/README.md) — FFI overview
- [Python Bindings](../ffi/python.md) — Python guide
