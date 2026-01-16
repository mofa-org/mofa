// Sample Rhai plugin
plugin_name = "my_hot_reload_plugin";
plugin_version = "1.0.0";
plugin_description = "A Rhai plugin with hot reload support";

// Counter state (will be preserved across reloads in the future)
let counter = 0;

// Initialize plugin
fn init() {
    print("Hot reload plugin initialized");
}

// Execute function - this is what gets called by the agent
fn execute(input) {
    let global_counter = counter;
    counter += 1;

    // Simple response
    {
        "message": "Hello from Rhai plugin!",
        "input": input,
        "counter": global_counter,
        "version": plugin_version
    }
}