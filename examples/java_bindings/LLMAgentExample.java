package com.mofa.examples;

import com.mofa.*;
import java.util.List;

/**
 * MoFA SDK Java Example - Basic LLM Agent
 *
 * This example demonstrates basic LLM agent usage including:
 * - Creating an agent with the builder pattern
 * - Simple Q&A (ask)
 * - Multi-turn chat
 * - Getting conversation history
 */
public class LLMAgentExample {

    public static void main(String[] args) {
        System.out.println("==================================================");
        System.out.println("MoFA SDK Java Example - Basic LLM Agent");
        System.out.println("==================================================");
        System.out.println();

        // Check for API key
        String apiKey = System.getenv("OPENAI_API_KEY");
        if (apiKey == null || apiKey.isEmpty()) {
            System.err.println("Error: OPENAI_API_KEY environment variable not set");
            System.err.println("Set it with: export OPENAI_API_KEY=your-key-here");
            System.exit(1);
        }

        try {
            // Create an LLM agent using the builder pattern
            System.out.println("1. Creating LLM Agent...");
            LLMAgentBuilder builder = UniFFI.INSTANCE.newLlmAgentBuilder();
            builder = builder.setId("my-agent");
            builder = builder.setName("Java Agent");
            builder = builder.setSystemPrompt("You are a helpful assistant.");
            builder = builder.setTemperature(0.7f);
            builder = builder.setMaxTokens(1000);

            String baseUrl = System.getenv("OPENAI_BASE_URL");
            String model = System.getenv().getOrDefault("OPENAI_MODEL", "gpt-3.5-turbo");
            builder = builder.setOpenaiProvider(apiKey, baseUrl, model);

            LLMAgent agent = builder.build();
            System.out.println("   Agent created: ID=" + agent.agentId() + ", Name=" + agent.name());
            System.out.println();

            // Simple Q&A (no context retention)
            System.out.println("2. Simple Q&A (ask)...");
            String question = "What is Java?";
            String answer = agent.ask(question);
            System.out.println("   Q: " + question);
            System.out.println("   A: " + answer);
            System.out.println();

            // Multi-turn chat (with context retention)
            System.out.println("3. Multi-turn chat...");
            String[] messages = {
                "My favorite color is blue.",
                "What did I just tell you?"
            };
            for (String msg : messages) {
                String response = agent.chat(msg);
                System.out.println("   User: " + msg);
                System.out.println("   Agent: " + response);
                System.out.println();
            }

            // Get conversation history
            System.out.println("4. Conversation history...");
            List<ChatMessage> history = agent.getHistory();
            System.out.println("   Total messages: " + history.size());
            for (int i = 0; i < history.size(); i++) {
                ChatMessage m = history.get(i);
                String preview = m.getContent().length() > 50
                    ? m.getContent().substring(0, 50) + "..."
                    : m.getContent();
                System.out.println("   [" + (i + 1) + "] " + m.getRole().name() + ": " + preview);
            }
            System.out.println();

            // Clear history
            System.out.println("5. Clearing history...");
            agent.clearHistory();
            history = agent.getHistory();
            System.out.println("   History after clear: " + history.size() + " messages");
            System.out.println();

        } catch (MoFaError e) {
            System.err.println("Error: " + e.getMessage());
            System.exit(1);
        }
    }
}
