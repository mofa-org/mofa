// MoFA SDK Go Example - Basic LLM Agent
//
// This example demonstrates basic LLM agent usage including:
// - Creating an agent with the builder pattern
// - Simple Q&A (ask)
// - Multi-turn chat
// - Getting conversation history
//
// Prerequisites:
// 1. Generate Go bindings: ./generate-go.sh
// 2. Set OPENAI_API_KEY environment variable
//
// Usage: go run 01_llm_agent.go

package main

import (
	"fmt"
	"os"

	// Import the generated Go bindings
	// Note: The actual import path will depend on the generated module structure
	mofa "mofa-sdk/bindings/go"
)

func main() {
	fmt.Println("==================================================")
	fmt.Println("MoFA SDK Go Example - Basic LLM Agent")
	fmt.Println("==================================================")
	fmt.Println()

	// Check for API key
	apiKey := os.Getenv("OPENAI_API_KEY")
	if apiKey == "" {
		fmt.Println("Error: OPENAI_API_KEY environment variable not set")
		fmt.Println("Set it with: export OPENAI_API_KEY=your-key-here")
		os.Exit(1)
	}

	baseURL := os.Getenv("OPENAI_BASE_URL")
	model := os.Getenv("OPENAI_MODEL")
	if model == "" {
		model = "gpt-3.5-turbo"
	}

	// Create an LLM agent using the builder pattern
	fmt.Println("1. Creating LLM Agent...")
	builder := mofa.NewLlmAgentBuilder()
	builder.SetId("my-agent")
	builder.SetName("Go Agent")
	builder.SetSystemPrompt("You are a helpful assistant.")
	builder.SetTemperature(0.7)
	builder.SetMaxTokens(1000)
	builder.SetOpenaiProvider(apiKey, baseURL, model)

	agent, err := builder.Build()
	if err != nil {
		fmt.Printf("Error building agent: %v\n", err)
		os.Exit(1)
	}

	agentID, _ := agent.AgentId()
	name, _ := agent.Name()
	fmt.Printf("   Agent created: ID=%s, Name=%s\n", agentID, name)
	fmt.Println()

	// Simple Q&A (no context retention)
	fmt.Println("2. Simple Q&A (ask)...")
	question := "What is Go?"
	answer, err := agent.Ask(question)
	if err != nil {
		fmt.Printf("Error: %v\n", err)
	} else {
		fmt.Printf("   Q: %s\n", question)
		fmt.Printf("   A: %s\n", answer)
	}
	fmt.Println()

	// Multi-turn chat (with context retention)
	fmt.Println("3. Multi-turn chat...")
	messages := []string{
		"My favorite color is blue.",
		"What did I just tell you?",
	}
	for _, msg := range messages {
		response, err := agent.Chat(msg)
		if err != nil {
			fmt.Printf("Error: %v\n", err)
		} else {
			fmt.Printf("   User: %s\n", msg)
			fmt.Printf("   Agent: %s\n", response)
		}
		fmt.Println()
	}

	// Get conversation history
	fmt.Println("4. Conversation history...")
	history := agent.GetHistory()
	fmt.Printf("   Total messages: %d\n", len(history))
	for i, msg := range history {
		preview := msg.Content
		if len(preview) > 50 {
			preview = preview[:50] + "..."
		}
		fmt.Printf("   [%d] %s: %s\n", i+1, msg.Role, preview)
	}
	fmt.Println()

	// Clear history
	fmt.Println("5. Clearing history...")
	agent.ClearHistory()
	history = agent.GetHistory()
	fmt.Printf("   History after clear: %d messages\n", len(history))
	fmt.Println()
}
