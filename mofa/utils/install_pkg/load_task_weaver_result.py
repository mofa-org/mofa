import json
from typing import Any, Dict, List, Optional


def extract_important_content(data: Dict[str, Any]) -> None:

    round_id: str = data.get("id", "N/A")
    user_query: str = data.get("user_query", "N/A")
    state: str = data.get("state", "N/A")

    print(f"=== Round ID: {round_id} ===",flush=True)
    print(f"User Query: {user_query}",flush=True)
    print(f"State: {state}\n",flush=True)

    post_list: List[Dict[str, Any]] = data.get("post_list", [])
    for idx, post in enumerate(post_list, start=1):
        post_id: str = post.get("id", "N/A")
        message: str = post.get("message", "N/A")
        send_from: str = post.get("send_from", "N/A")
        send_to: str = post.get("send_to", "N/A")
        attachments: List[Dict[str, Any]] = post.get("attachment_list", [])

        print(f"--- Post {idx} ---",flush=True)
        # print(f"Post ID: {post_id}")
        print(f"Message: {message}",flush=True)
        print(f"From: {send_from}",flush=True)
        print(f"To: {send_to}",flush=True)

        if attachments:
            print("Attachments:",flush=True)
            for att_idx, attachment in enumerate(attachments, start=1):
                att_id: str = attachment.get("id", "N/A")
                att_type: str = attachment.get("type", "N/A")
                att_content: Any = attachment.get("content", "N/A")
                att_extra: Optional[Dict[str, Any]] = attachment.get("extra", None)

                print(f"  Attachment {att_idx}:",flush=True)
                # print(f"    ID: {att_id}")
                print(f"    Type: {att_type}",flush=True)
                print(f"    Content: {att_content}",flush=True)

                if att_extra:
                    print(f"    Extra:",flush=True)
                    for key, value in att_extra.items():
                        print(f"      {key}: {value}",flush=True)
        else:
            print("Attachments: None",flush=True)
        print("\n",flush=True)


def main():
    # 示例输入数据（请根据实际情况替换为您的数据）
    data = {
        "id": "round-20240923-065454-46d45ab3",
        "user_query": "明天的天气",
        "state": "finished",
        "post_list": [
            {
                "id": "post-20240923-065454-e566325e",
                "message": "明天的天气",
                "send_from": "User",
                "send_to": "Planner",
                "attachment_list": []
            },
            {
                "id": "post-20240923-065454-4d648811",
                "message": "请问您想查询哪个地区明天的天气？",
                "send_from": "Planner",
                "send_to": "User",
                "attachment_list": [
                    {
                        "id": "atta-20240923-065456-2046ceb5",
                        "type": "init_plan",
                        "content": "1. Respond to the user's request about tomorrow's weather",
                        "extra": None
                    },
                    {
                        "id": "atta-20240923-065456-37bfd467",
                        "type": "plan",
                        "content": "1. Respond to the user's request about tomorrow's weather",
                        "extra": None
                    },
                    {
                        "id": "atta-20240923-065457-90c2debe",
                        "type": "current_plan_step",
                        "content": "1. Respond to the user's request about tomorrow's weather",
                        "extra": None
                    },
                    {
                        "id": "atta-20240923-065458-11111747",
                        "type": "shared_memory_entry",
                        "content": "Add the plan to the shared memory",
                        "extra": {
                            "type": "plan",
                            "content": "\n====== Plan ======\nI have drawn up a plan:\n1. Respond to the user's request about tomorrow's weather\n==================\n",
                            "scope": "round",
                            "id": "sme-20240923-065458-2624886d"
                        }
                    }
                ]
            }
        ]
    }

    # 调用提取函数
    extract_important_content(data)

if __name__ == "__main__":
    main()