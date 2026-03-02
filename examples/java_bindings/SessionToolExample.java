package com.mofa.examples;

import com.mofa.*;
import java.util.List;

/**
 * MoFA SDK Java Example - Session Management and Tool Registration
 *
 * This example demonstrates:
 * - Creating and managing conversation sessions
 * - Defining custom tools in Java
 * - Registering tools and executing them through the registry
 *
 * No LLM API key is required for this example.
 */
public class SessionToolExample {

    public static void main(String[] args) {
        System.out.println("==================================================");
        System.out.println("MoFA SDK Java Example - Sessions & Tools");
        System.out.println("==================================================");
        System.out.println();

        try {
            sessionDemo();
            toolDemo();
        } catch (MoFaError e) {
            System.err.println("Error: " + e.getMessage());
            System.exit(1);
        }
    }

    static void sessionDemo() throws MoFaError {
        System.out.println("--- Session Management ---");

        // Create an in-memory session manager
        SessionManager manager = SessionManager.newInMemory();

        // Create a session
        Session session = manager.getOrCreate("java-chat");
        System.out.println("Created session: " + session.getKey());

        // Add messages
        session.addMessage("user", "Hello from Java!");
        session.addMessage("assistant", "Hi! How can I help you today?");
        session.addMessage("user", "Tell me about MoFA.");
        System.out.println("Messages: " + session.messageCount());

        // Retrieve history
        List<SessionMessageInfo> history = session.getHistory(10);
        for (SessionMessageInfo msg : history) {
            System.out.println("  [" + msg.getRole() + "] " + msg.getContent());
        }

        // Save and list
        manager.saveSession(session);
        List<String> keys = manager.listSessions();
        System.out.println("Sessions: " + keys);

        // Metadata
        session.setMetadata("language", "\"Java\"");
        String lang = session.getMetadata("language");
        System.out.println("Metadata language: " + lang);

        System.out.println();
    }

    static void toolDemo() throws MoFaError {
        System.out.println("--- Tool Registration ---");

        ToolRegistry registry = new ToolRegistry();

        // Register a calculator tool
        registry.registerTool(new FfiToolCallback() {
            @Override
            public String name() {
                return "multiply";
            }

            @Override
            public String description() {
                return "Multiply two numbers";
            }

            @Override
            public String parametersSchemaJson() {
                return "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"number\"},\"b\":{\"type\":\"number\"}},\"required\":[\"a\",\"b\"]}";
            }

            @Override
            public FfiToolResult execute(String argumentsJson) {
                try {
                    // Simple JSON parsing (in production, use a proper JSON library)
                    String clean = argumentsJson.replaceAll("[{}\"]", "");
                    String[] parts = clean.split(",");
                    double a = 0, b = 0;
                    for (String part : parts) {
                        String[] kv = part.split(":");
                        if (kv[0].trim().equals("a")) a = Double.parseDouble(kv[1].trim());
                        if (kv[0].trim().equals("b")) b = Double.parseDouble(kv[1].trim());
                    }
                    double result = a * b;
                    return new FfiToolResult(true, String.valueOf(result), null);
                } catch (Exception e) {
                    return new FfiToolResult(false, "null", e.getMessage());
                }
            }
        });

        System.out.println("Registered tools: " + registry.listToolNames());
        System.out.println("Tool count: " + registry.toolCount());

        // Execute the tool
        FfiToolResult result = registry.executeTool("multiply", "{\"a\": 6, \"b\": 7}");
        System.out.println("6 * 7 = " + result.getOutputJson() + " (success: " + result.getSuccess() + ")");

        System.out.println();
    }
}
