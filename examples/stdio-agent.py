#!/usr/bin/env python3
"""Protocol fixture for manual UI testing; this is not an AI Agent."""

import json
import sys
import time
import uuid


def emit(event):
    print(json.dumps(event, ensure_ascii=False), flush=True)


request = json.load(sys.stdin)
session_id = request.get("session_id") or f"fixture-{uuid.uuid4()}"
emit({"type": "session", "id": session_id})
part_id = str(uuid.uuid4())
emit({"type": "text", "id": part_id, "text": "这是 **协议测试程序** 的回复，用于验证界面和事件流。\n\n", "delta": True})
time.sleep(0.3)
emit({"type": "tool", "id": str(uuid.uuid4()), "name": "协议测试 · 接收到的工作目录", "details": request["directory"]})
for text in ["消息已收到：", request["prompt"], "\n\n会话 ID：`" + session_id + "`"]:
    emit({"type": "text", "id": part_id, "text": text, "delta": True})
    time.sleep(0.3)
