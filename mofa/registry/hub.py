"""Hub client for fetching remote registry"""

import os
import json
import requests
from typing import List, Dict, Optional
from pathlib import Path


class HubClient:
    """Client for interacting with mofa-node-hub"""

    DEFAULT_HUB_URL = "https://raw.githubusercontent.com/mofa-org/mofa-node-hub/main"
    CACHE_DIR = Path.home() / ".mofa" / "cache"
    CACHE_FILE = CACHE_DIR / "registry.json"
    CACHE_TTL = 3600  # 1 hour

    def __init__(self, hub_url: Optional[str] = None):
        self.hub_url = hub_url or os.getenv('MOFA_HUB_URL', self.DEFAULT_HUB_URL)
        self.registry_url = f"{self.hub_url}/registry.json"
        self.CACHE_DIR.mkdir(parents=True, exist_ok=True)

    def _is_cache_valid(self) -> bool:
        """Check if cache exists and is still valid"""
        if not self.CACHE_FILE.exists():
            return False

        cache_age = Path(self.CACHE_FILE).stat().st_mtime
        import time
        return (time.time() - cache_age) < self.CACHE_TTL

    def _fetch_registry(self) -> Dict:
        """Fetch registry from remote hub"""
        try:
            response = requests.get(self.registry_url, timeout=10)
            response.raise_for_status()
            return response.json()
        except Exception as e:
            raise RuntimeError(f"Failed to fetch registry from hub: {e}")

    def _save_cache(self, data: Dict):
        """Save registry to cache"""
        with open(self.CACHE_FILE, 'w') as f:
            json.dump(data, f, indent=2)

    def _load_cache(self) -> Dict:
        """Load registry from cache"""
        with open(self.CACHE_FILE, 'r') as f:
            return json.load(f)

    def get_registry(self, use_cache: bool = True) -> Dict:
        """Get registry data (from cache or remote)"""
        if use_cache and self._is_cache_valid():
            return self._load_cache()

        # Fetch from remote
        registry = self._fetch_registry()
        self._save_cache(registry)
        return registry

    def list_agents(self, use_cache: bool = True) -> List[Dict]:
        """List all agents from hub"""
        registry = self.get_registry(use_cache)
        return registry.get('agents', [])

    def list_flows(self, use_cache: bool = True) -> List[Dict]:
        """List all flows from hub"""
        registry = self.get_registry(use_cache)
        return registry.get('flows', [])

    def search_agents(self, keyword: str, use_cache: bool = True) -> List[Dict]:
        """Search agents by keyword"""
        agents = self.list_agents(use_cache)
        keyword_lower = keyword.lower()
        return [
            agent for agent in agents
            if keyword_lower in agent.get('name', '').lower()
            or keyword_lower in agent.get('description', '').lower()
            or keyword_lower in ' '.join(agent.get('tags', [])).lower()
        ]

    def search_flows(self, keyword: str, use_cache: bool = True) -> List[Dict]:
        """Search flows by keyword"""
        flows = self.list_flows(use_cache)
        keyword_lower = keyword.lower()
        return [
            flow for flow in flows
            if keyword_lower in flow.get('name', '').lower()
            or keyword_lower in flow.get('description', '').lower()
        ]
