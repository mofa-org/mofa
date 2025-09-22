import os
from typing import Dict, Any, List, Optional, Union
from attrs import define, field
from dotenv import load_dotenv
from mem0 import Memory

from mofa.utils.files.read import read_yaml


@define
class MemoryAgent:
    """
    完整的内存管理代理，支持增删改查等所有操作
    基于 mem0 开源版本实现
    """
    config_path: str = field()
    llm_client: Optional[Memory] = field(default=None, init=False)
    user_id: Optional[str] = field(default=None, init=False)


    def __attrs_post_init__(self):
        """初始化时自动创建LLM客户端和加载用户ID"""
        self._setup_environment()
        self._load_user_id()
        self._create_llm_client()
        self._load_proxy

    @property
    def _load_proxy(self):
        """
        加载代理配置并设置环境变量

        代理配置应在 config.yaml 的 system.proxy 节点下定义，例如：
        system:
          proxy:
            http: http://127.0.0.1:8080
            https: http://127.0.0.1:8080
        """
        config = read_yaml(file_path=self.config_path)
        proxy_config = config.get('system', {}).get('proxy', None)
        if proxy_config:
            http_proxy = proxy_config.get("http")
            https_proxy = proxy_config.get("https")
            if http_proxy:
                os.environ["http_proxy"] = http_proxy
            if https_proxy:
                os.environ["https_proxy"] = https_proxy


    def _setup_environment(self):
        """设置环境变量"""
        load_dotenv(override=True)
        os.environ["OPENAI_API_KEY"] = os.environ.get('LLM_API_KEY', '')
        if os.environ.get('LLM_API_URL'):
            os.environ["OPENAI_API_BASE"] = os.environ.get('LLM_API_URL')

    def _load_config(self) -> Dict[str, Any]:
        """加载配置"""
        return read_yaml(file_path=self.config_path)['agent']['llm']

    def _load_user_id(self):
        """加载用户ID"""
        self.user_id = read_yaml(file_path=self.config_path)['agent']['user_id']

    def _create_llm_client(self):
        """创建Memory客户端"""
        config = self._load_config()
        self.llm_client = Memory.from_config(config)

    def _extract_memories(self, memory_data) -> List[str]:
        """从内存数据中提取记忆文本"""
        try:
            if isinstance(memory_data, list):
                return list(set([item.get("memory") for item in memory_data if item.get("memory")]))
            elif isinstance(memory_data, dict):
                return memory_data.get("results", [])
        except Exception:
            return []
        return []

    def _validate_client(self):
        """验证客户端是否正确初始化"""
        if not self.llm_client or not self.user_id:
            raise ValueError("LLM client or user_id not properly initialized")

    # ==================== 增加记忆 ====================
    def add(self,
            data: Union[str, List[Dict[str, str]]],
            user_id: Optional[str] = None,
            agent_id: Optional[str] = None,
            run_id: Optional[str] = None,
            metadata: Optional[Dict[str, Any]] = None,
            infer: bool = True) -> Dict[str, Any]:
        """
        添加新的记忆

        Args:
            data: 要存储的数据，可以是字符串或消息列表
            user_id: 用户ID，如果不提供则使用默认用户ID
            agent_id: 代理ID
            run_id: 运行会话ID
            metadata: 元数据
            infer: 是否推断记忆（默认True）

        Returns:
            Dict: 添加操作的结果
        """
        self._validate_client()

        kwargs = {
            "user_id": user_id or self.user_id,
            "infer": infer
        }

        if agent_id:
            kwargs["agent_id"] = agent_id
        if run_id:
            kwargs["run_id"] = run_id
        if metadata:
            kwargs["metadata"] = metadata

        return self.llm_client.add(data, **kwargs)

    def add_messages(self,
                     messages: List[Dict[str, str]],
                     user_id: Optional[str] = None,
                     **kwargs) -> Dict[str, Any]:
        """
        添加对话消息记忆

        Args:
            messages: 对话消息列表
            user_id: 用户ID
            **kwargs: 其他参数

        Returns:
            Dict: 添加操作的结果
        """
        return self.add(messages, user_id=user_id, **kwargs)

    # ==================== 搜索记忆 ====================
    def search(self,
               query: str,
               user_id: Optional[str] = None,
               agent_id: Optional[str] = None,
               run_id: Optional[str] = None,
               limit: int = 10) -> List[str]:
        """
        搜索记忆

        Args:
            query: 搜索查询
            user_id: 用户ID
            agent_id: 代理ID
            run_id: 运行会话ID
            limit: 返回结果数量限制

        Returns:
            List[str]: 搜索到的记忆列表
        """
        self._validate_client()

        kwargs = {
            "user_id": user_id or self.user_id,
            "limit": limit
        }

        if agent_id:
            kwargs["agent_id"] = agent_id
        if run_id:
            kwargs["run_id"] = run_id

        memory_result = self.llm_client.search(query, **kwargs)
        return self._extract_memories(memory_result)

    # ==================== 获取记忆 ====================
    def get_all(self,
                user_id: Optional[str] = None,
                agent_id: Optional[str] = None,
                run_id: Optional[str] = None,
                limit: Optional[int] = None) -> List[Dict[str, Any]]:
        """
        获取所有记忆

        Args:
            user_id: 用户ID
            agent_id: 代理ID
            run_id: 运行会话ID
            limit: 返回结果数量限制

        Returns:
            List[Dict]: 所有记忆列表
        """
        self._validate_client()

        kwargs = {
            "user_id": user_id or self.user_id
        }

        if agent_id:
            kwargs["agent_id"] = agent_id
        if run_id:
            kwargs["run_id"] = run_id
        if limit:
            kwargs["limit"] = limit

        return self.llm_client.get_all(**kwargs)

    def get(self, memory_id: str) -> Dict[str, Any]:
        """
        根据ID获取特定记忆

        Args:
            memory_id: 记忆ID

        Returns:
            Dict: 记忆详情
        """
        self._validate_client()
        return self.llm_client.get(memory_id)

    # ==================== 更新记忆 ====================
    def update(self, memory_id: str, data: str) -> Dict[str, Any]:
        """
        更新记忆

        Args:
            memory_id: 要更新的记忆ID
            data: 新的记忆内容

        Returns:
            Dict: 更新操作的结果
        """
        self._validate_client()
        return self.llm_client.update(memory_id, data)

    # ==================== 删除记忆 ====================
    def delete(self, memory_id: str) -> Dict[str, Any]:
        """
        删除特定记忆

        Args:
            memory_id: 要删除的记忆ID

        Returns:
            Dict: 删除操作的结果
        """
        self._validate_client()
        return self.llm_client.delete(memory_id)

    def delete_all(self,
                   user_id: Optional[str] = None,
                   agent_id: Optional[str] = None,
                   run_id: Optional[str] = None) -> Dict[str, Any]:
        """
        删除指定条件下的所有记忆

        Args:
            user_id: 用户ID
            agent_id: 代理ID
            run_id: 运行会话ID

        Returns:
            Dict: 删除操作的结果
        """
        self._validate_client()

        kwargs = {
            "user_id": user_id or self.user_id
        }

        if agent_id:
            kwargs["agent_id"] = agent_id
        if run_id:
            kwargs["run_id"] = run_id

        return self.llm_client.delete_all(**kwargs)

    def reset(self) -> Dict[str, Any]:
        """
        重置所有记忆（完全清空）

        Returns:
            Dict: 重置操作的结果
        """
        self._validate_client()
        return self.llm_client.reset()

    # ==================== 记忆历史 ====================
    def history(self, memory_id: str) -> List[Dict[str, Any]]:
        """
        获取记忆的历史变更记录

        Args:
            memory_id: 记忆ID

        Returns:
            List[Dict]: 历史记录列表
        """
        self._validate_client()
        return self.llm_client.history(memory_id)

    # ==================== 辅助方法 ====================
    def get_memory_count(self,
                         user_id: Optional[str] = None,
                         agent_id: Optional[str] = None) -> int:
        """
        获取记忆数量

        Args:
            user_id: 用户ID
            agent_id: 代理ID

        Returns:
            int: 记忆数量
        """
        memories = self.get_all(user_id=user_id, agent_id=agent_id)
        return len(memories)

    def search_by_metadata(self,
                           metadata_key: str,
                           metadata_value: str,
                           user_id: Optional[str] = None) -> List[Dict[str, Any]]:
        """
        根据元数据搜索记忆

        Args:
            metadata_key: 元数据键
            metadata_value: 元数据值
            user_id: 用户ID

        Returns:
            List[Dict]: 匹配的记忆列表
        """
        all_memories = self.get_all(user_id=user_id)
        return [
            memory for memory in all_memories
            if memory.get('metadata', {}).get(metadata_key) == metadata_value
        ]

if __name__ == "__main__":

    # 示例用法

    # 初始化
    agent = MemoryAgent(config_path="config.yaml")

    # 添加记忆
    result = agent.add("用户喜欢吃川菜", metadata={"category": "food_preference"})

    # 添加对话记忆
    messages = [
        {"role": "user", "content": "我想看科幻电影"},
        {"role": "assistant", "content": "推荐《星际穿越》给你"}
    ]
    add_data = agent.add_messages(messages, run_id="session-001")
    memory_id = add_data['results'][0]['id']
    # 搜索记忆
    memories = agent.search("用户喜欢什么食物", limit=5)
    print('memories:', memories)
    # 获取所有记忆
    all_memories = agent.get_all()
    print('all_memories:', all_memories)
    # 更新记忆
    agent.update(memory_id=memory_id, data="用户特别喜欢麻辣川菜")

    # 删除记忆
    agent.delete(memory_id=memory_id)

    # 获取记忆统计
    count = agent.get_memory_count()