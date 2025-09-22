# import json
# import time
# import uuid
# import os
# import asyncio
# from typing import AsyncGenerator
# import traceback
# import uvicorn
# from fastapi import FastAPI, HTTPException
# from fastapi.middleware.cors import CORSMiddleware
# from fastapi.responses import StreamingResponse
# from dotenv import load_dotenv
# import pyarrow as pa
# from pydantic import BaseModel
# from typing import List, Optional
#
# from dora import Node  # Dora 节点，用于节点间通信
#
# # 加载环境变量
# load_dotenv('.env.secret')
# DORA_RESPONSE_TIMEOUT = 180  # 等待节点响应的超时时间
#
# app = FastAPI(title="Dora Streaming API with Dora Integration")
#
# origins = ["*"]
# app.add_middleware(
#     CORSMiddleware,
#     allow_origins=origins,
#     allow_credentials=True,
#     allow_methods=["*"],
#     allow_headers=["*"],
# )
#
#
# # 定义请求消息结构
# class ChatCompletionMessage(BaseModel):
#     role: str  # 例如 "user" 或 "assistant"
#     content: str
#
#
# # 定义请求结构
# class ChatCompletionRequest(BaseModel):
#     model: str  # 模型名称，例如 "gpt-4o-mini"
#     messages: List[ChatCompletionMessage]
#     temperature: Optional[float] = 1.0
#     max_tokens: Optional[int] = 100
#
#
# # 定义响应结构（仅供参考）
# class ChatCompletionResponse(BaseModel):
#     id: str
#     object: str
#     created: int
#     model: str
#     choices: List[dict]
#     usage: dict
#
#
# # 初始化 Dora 节点
# node = Node()
#
#
# def clean_string(input_string: str) -> str:
#     return input_string.encode('utf-8', 'replace').decode('utf-8')
#
#
#
# # 独立的流式数据生成函数（带 Dora）
# async def dora_event_stream(request_model: str) -> AsyncGenerator[str, None]:
#     """
#     持续轮询 Dora 节点返回的事件，
#     如果收到事件类型为 "INPUT" 且 id 为 "v3/chat/completions"，
#     则解析 event["value"][0]（一个单独的块），取出 'node_results' 字段，
#     构造符合 OpenAI 流式返回格式的块返回给客户端，
#     如果解析结果中 type 为 "completion"，则表示最后一块，退出循环。
#     """
#     while True:
#         event = node.next(timeout=DORA_RESPONSE_TIMEOUT)
#         if event["type"] == "INPUT" and event["id"] == "v3/chat/completions":
#             response_val = event["value"][0].as_py()
#             parsed = json.loads(response_val)
#             parsed = json.loads(parsed['node_results'])
#             finish_reason = ''
#             if parsed.get("end",None) is not None:
#                 finish_reason = "stop"
#             stream_chunk = {
#                 "id": str(uuid.uuid4()),
#                 "object": "chat.completion.chunk",
#                 "created": int(time.time()),
#                 "model": request_model,
#                 "choices": [{
#                     "delta": {
#                         "content": parsed.get("content", ""),
#                         "articles": parsed.get("articles", []),
#                         "metadata": parsed.get("metadata", {}),
#                         'type': parsed.get('type', 'content'),
#                         'id': parsed.get('id', 0),
#                     },
#                     "index": 0,
#                     "finish_reason": finish_reason
#                 }]
#             }
#             yield "data: " + json.dumps(stream_chunk) + "\n\n"
#             if finish_reason == "stop":
#                 break
#         else:
#             await asyncio.sleep(0.1)
#
#
# @app.post("/v3/chat/completions")
# async def create_chat_completion(request: ChatCompletionRequest):
#     user_query = next((msg.content for msg in request.messages if msg.role == "user"), "")
#     if not user_query:
#         raise HTTPException(status_code=400, detail="No user query provided")
#
#     data = pa.array([clean_string(user_query)])
#     node.send_output("v3/chat/completions", data)
#     return StreamingResponse(dora_event_stream(request.model), media_type="text/event-stream")
#
#
# @app.get("/v3/models")
# async def list_models():
#     return {
#         "object": "list",
#         "data": [
#             {
#                 "id": "gpt-4o-mini",
#                 "object": "model",
#                 "created": 1677610602,
#                 "owned_by": "openai",
#             }
#         ],
#     }
#
#
# @app.get("/v3/hello")
# async def hello():
#     return "Hello World"
# async def run_fastapi():
#     config = uvicorn.Config(app, host="0.0.0.0", port=8000, log_level="info")
#     server = uvicorn.Server(config)
#
#     server = asyncio.gather(server.serve())
#     while True:
#         await asyncio.sleep(1)
#         event = node.next(0.001)
#         if event["type"] == "STOP":
#             break
#
# def main():
#     asyncio.run(run_fastapi())
# if __name__ == "__main__":
#     asyncio.run(run_fastapi())


import json
import time
import uuid
import os
import asyncio
from typing import AsyncGenerator
import traceback
import uvicorn
from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse
from dotenv import load_dotenv
import pyarrow as pa
from pydantic import BaseModel
from typing import List, Optional

from dora import Node  # Dora 节点，用于节点间通信

# 加载环境变量
load_dotenv('.env.secret')
DORA_RESPONSE_TIMEOUT = 180  # 等待节点响应的超时时间

app = FastAPI(title="Dora Streaming API with Dora Integration")

origins = ["*"]
app.add_middleware(
    CORSMiddleware,
    allow_origins=origins,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


# 定义请求消息结构
class ChatCompletionMessage(BaseModel):
    role: str  # 例如 "user" 或 "assistant"
    content: str


# 定义请求结构
class ChatCompletionRequest(BaseModel):
    model: str  # 模型名称，例如 "gpt-4o-mini"
    messages: List[ChatCompletionMessage]
    temperature: Optional[float] = 1.0
    max_tokens: Optional[int] = 100


# 定义响应结构（仅供参考）
class ChatCompletionResponse(BaseModel):
    id: str
    object: str
    created: int
    model: str
    choices: List[dict]
    usage: dict


# 初始化 Dora 节点
node = Node()


def clean_string(input_string: str) -> str:
    return input_string.encode('utf-8', 'replace').decode('utf-8')


# 改造后的流式数据生成函数，支持客户端断开检测
async def dora_event_stream(request_model: str, request: Request) -> AsyncGenerator[str, None]:
    """
    持续轮询 Dora 节点返回的事件，
    支持检测客户端断开连接，优雅退出。
    """
    try:
        while True:
            # 客户端断开检测
            if await request.is_disconnected():
                print("Client disconnected, stopping stream.")
                break

            event = node.next(timeout=DORA_RESPONSE_TIMEOUT)
            if event["type"] == "INPUT" and event["id"] == "v3/chat/completions":
                response_val = event["value"][0].as_py()
                parsed = json.loads(response_val)
                parsed = json.loads(parsed['node_results'])
                finish_reason = ''
                if parsed.get("end", None) is not None:
                    finish_reason = "stop"

                stream_chunk = {
                    "id": str(uuid.uuid4()),
                    "object": "chat.completion.chunk",
                    "created": int(time.time()),
                    "model": request_model,
                    "choices": [{
                        "delta": {
                            "content": parsed.get("content", ""),
                            "articles": parsed.get("articles", []),
                            "metadata": parsed.get("metadata", {}),
                            "type": parsed.get("type", "content"),
                            "id": parsed.get("id", 0),
                        },
                        "index": 0,
                        "finish_reason": finish_reason
                    }]
                }
                yield "data: " + json.dumps(stream_chunk) + "\n\n"
                if finish_reason == "stop":
                    break
            else:
                await asyncio.sleep(0.1)
    except Exception as e:
        # 可以根据需要自定义日志或监控
        print("Stream generator error:", traceback.format_exc())
    finally:
        print("Stream generator exited.")


@app.post("/v3/chat/completions")
async def create_chat_completion(request: Request, body: ChatCompletionRequest):
    user_query = next((msg.content for msg in body.messages if msg.role == "user"), "")
    if not user_query:
        raise HTTPException(status_code=400, detail="No user query provided")

    data = pa.array([clean_string(user_query)])
    node.send_output("v3/chat/completions", data)

    return StreamingResponse(
        dora_event_stream(body.model, request),
        media_type="text/event-stream"
    )


@app.get("/v3/models")
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


@app.get("/v3/hello")
async def hello():
    return "Hello World"


async def run_fastapi():
    config = uvicorn.Config(app, host="0.0.0.0", port=8000, log_level="info")
    server = uvicorn.Server(config)

    server_task = asyncio.create_task(server.serve())
    try:
        while True:
            await asyncio.sleep(1)
            event = node.next(0.001)
            if event.get("type") == "STOP":
                break
    finally:
        server.should_exit = True
        await server_task


def main():
    asyncio.run(run_fastapi())

if __name__ == "__main__":
    main()
