import json
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware  # 需要导入 CORSMiddleware
from pydantic import BaseModel  # Pydantic is used for request and response validation
from typing import List, Optional  # Type hinting for request and response data
import uvicorn  # Uvicorn for running ASGI servers
from dora import Node  # Dora for node communication in a dataflow system
import asyncio  # Asynchronous I/O operations
import pyarrow as pa  # PyArrow for data serialization and transfer
import ast  # Abstract Syntax Trees for evaluating user input
from datetime import datetime

# Timeout duration for waiting on responses from Dora nodes
DORA_RESPONSE_TIMEOUT = 180

app = FastAPI()

origins = [
    "*",  # Allow all origins (be careful with this in production)
]

# Add CORSMiddleware to allow CORS on specific routes
app.add_middleware(
    CORSMiddleware,
    allow_origins=origins,  # Allows access from the origins listed above
    allow_credentials=True,
    allow_methods=["*"],  # Allows all HTTP methods (GET, POST, etc.)
    allow_headers=["*"],  # Allows all headers
)


# Define the structure for a chat message in the chat completion request,
# which contain the role (e.g., user, assistant) and the message content.
class ChatCompletionMessage(BaseModel):
    role: str  # Role of the message (e.g., 'user', 'assistant')
    content: str  # Content of the message


# Define the structure for a chat completion request, containing a model type,
# a list of messages, temperature, and max tokens.
class ChatCompletionRequest(BaseModel):
    model: str  # The model to be used for the chat completion (e.g., 'gpt-3.5-turbo')
    messages: List[ChatCompletionMessage]  # List of chat messages
    temperature: Optional[float] = 1.0  # Optional: Sampling temperature (defaults to 1.0)
    max_tokens: Optional[int] = 100  # Optional: Maximum number of tokens (defaults to 100)


class ChatCompletionResponse(BaseModel):
    id: str  # Unique identifier for the completion response
    object: str  # Object type (e.g., 'chat.completion')
    created: int  # Timestamp of creation
    model: str  # Model used for response
    choices: List[dict]  # List of choices in the response (each with index, message, etc.)
    usage: dict  # Usage statistics (e.g., prompt_tokens, completion_tokens, total_tokens)


node = Node()


def clean_string(input_string: str):
    return input_string.encode('utf-8', 'replace').decode('utf-8')


# Defines a POST endpoint /v1/chat/completions to handle chat completion requests.
@app.post("/v1/chat/completions")
async def create_chat_completion(request: ChatCompletionRequest):
    """
    - Accepts a ChatCompletionRequest as input.
    - Extracts user messages and converts them to a suitable PyArrow format.
    - Sends the message to a Dora node for processing.
    - Waits for a response and returns it in a structured format (with token usage and completion).
    """

    # Extracts the user's message from the list of messages and converts it into a PyArrow array for efficient serialization.
    data = next(
        (msg.content for msg in request.messages if msg.role == "user"),
        "No user message found.",
    )

    # Convert the data into a PyArrow array for efficient data processing
    data = pa.array([clean_string(data)])
    now = datetime.now()
    formatted_time = now.strftime("%Y-%m-%d %H:%M:%S")

    print(formatted_time,'      received data:', data, )
    # The message is then sent to the next node in the dataflow system with the output label 'v1/chat/completions'
    node.send_output("v1/chat/completions", data)
    print('发送完毕')
    # Dora Node Event Loop: Wait for response from the next node in the dataflow system.
    while True:
        # Get the next event from the node (with a timeout)
        event = node.next(timeout=DORA_RESPONSE_TIMEOUT)
        # Check if the event is an error or an input event for chat completions
        # if event["type"] == "ERROR":
        #     response_str = "No response received. Err: "
        #     break
        # If the event is an input event with the expected ID, process the response
        if  event["type"] == "INPUT" and event["id"] == "v1/chat/completions":
            response = event["value"]
            # Extract the first element of the response or set a default message if no response is received
            response_str = response[0].as_py() if response else "No response received"
            if "No response received" != response_str:
                try:
                    response_str = json.loads(response_str)
                    response_str = response_str['node_results']
                except Exception as e :
                    pass

            break
        else:
            pass

    # Return the chat completion response with the processed response message and token usage statistics
    return ChatCompletionResponse(
        id="chatcmpl-1234",  # Unique identifier for the completion response, sample value
        object="chat.completion",  # Object type
        created=1234567890,  # Timestamp of creation, sample value
        model=request.model,  # Model used for response
        choices=[
            {
                "index": 0,
                "message": {"role": "assistant", "content": response_str},
                "finish_reason": "stop",
            }
        ],
        # Provide token usage stats for the prompt and completion
        usage={
            "prompt_tokens": len(data),  # Number of tokens in the prompt
            "completion_tokens": len(response_str),  # Number of tokens in the completion
            "total_tokens": len(data) + len(response_str),  # Total number of tokens
        },
    )


# List Models Endpoint: Provides a simple GET endpoint /v1/models that returns a static list of models (e.g., "gpt-3.5-turbo").
@app.get("/v1/models")
async def list_models():
    return {
        "object": "list",
        "data": [
            {
                "id": "gpt-4o-mini",
                "object": "model",
                "created": 1677610602,
                "owned_by": "openai",
            }
        ],
    }
@app.get("/v1/hello")
async def hello():
    return "Hello World"
# Starts the FastAPI server asynchronously using Uvicorn and also processes the events from the Dora node concurrently.
async def run_fastapi():
    # Configure the Uvicorn server with host, port, and log level settings
    # 127.0.0.1 for localhost, local-only access
    # 0.0.0.0 for all available network interfaces
    config = uvicorn.Config(app, host="0.0.0.0", port=8000, log_level="info")
    server = uvicorn.Server(config)

    # Start the Uvicorn server and run it asynchronously
    server = asyncio.gather(server.serve())
    # Keep checking for Dora node events while the server is running
    while True:
        # Sleep for 1 second between checks to avoid blocking the event loop
        await asyncio.sleep(1)
        # Called repeatedly to keep checking for any incoming events in the Dora dataflow system.
        event = node.next(0.001)
        # If a stop event is received, exit the loop and stop the server
        if event["type"] == "STOP":
            break


# Starts the FastAPI server and runs the event loop to handle incoming requests and dataflow events.
def main():
    # Run the FastAPI server asynchronously
    asyncio.run(run_fastapi())


# If the script is run directly, start the FastAPI server and event loop
if __name__ == "__main__":
    asyncio.run(run_fastapi())
