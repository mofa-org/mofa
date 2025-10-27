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

    def download_agent(self, name: str, output_dir: str) -> bool:
        """Download an agent from hub using git sparse-checkout"""
        import subprocess
        import tempfile
        import shutil

        # Get agent info from registry
        agents = self.list_agents(use_cache=False)
        agent = next((a for a in agents if a.get('name') == name), None)

        if not agent:
            raise ValueError(f"Agent '{name}' not found in hub")

        # Get path from registry
        agent_path = agent.get('path')
        if not agent_path:
            raise ValueError(f"No path information for agent '{name}'")

        # Check if already exists
        output_path = Path(output_dir) / name
        if output_path.exists():
            raise FileExistsError(f"Agent '{name}' already exists at {output_path}")

        # Get hub URL from registry or use default
        registry = self.get_registry(use_cache=False)
        repo_url = registry.get('hub_url', 'https://github.com/mofa-org/mofa-node-hub')

        # Create temp directory for sparse checkout
        temp_dir = tempfile.mkdtemp(prefix='mofa_download_')

        try:
            # Initialize git repo
            subprocess.run(
                ['git', 'init'],
                cwd=temp_dir,
                capture_output=True,
                check=True
            )

            # Add remote
            subprocess.run(
                ['git', 'remote', 'add', 'origin', repo_url],
                cwd=temp_dir,
                capture_output=True,
                check=True
            )

            # Enable sparse checkout
            subprocess.run(
                ['git', 'config', 'core.sparseCheckout', 'true'],
                cwd=temp_dir,
                capture_output=True,
                check=True
            )

            # Specify which directory to checkout
            sparse_checkout_file = Path(temp_dir) / '.git' / 'info' / 'sparse-checkout'
            sparse_checkout_file.parent.mkdir(parents=True, exist_ok=True)
            with open(sparse_checkout_file, 'w') as f:
                f.write(f"{agent_path}\n")

            # Pull the specific directory
            subprocess.run(
                ['git', 'pull', 'origin', 'main', '--depth=1'],
                cwd=temp_dir,
                capture_output=True,
                check=True
            )

            # Copy the downloaded agent to output directory
            source = Path(temp_dir) / agent_path
            if not source.exists():
                raise RuntimeError(f"Agent directory not found after checkout: {source}")

            shutil.copytree(source, output_path)

            return True

        except subprocess.CalledProcessError as e:
            raise RuntimeError(f"Git operation failed: {e.stderr.decode() if e.stderr else str(e)}")
        except Exception as e:
            # Clean up on failure
            if output_path.exists():
                shutil.rmtree(output_path, ignore_errors=True)
            raise RuntimeError(f"Failed to download agent '{name}': {e}")
        finally:
            # Clean up temp directory
            if Path(temp_dir).exists():
                shutil.rmtree(temp_dir, ignore_errors=True)

    def download_flow(self, name: str, output_dir: str) -> bool:
        """Download a flow from hub using git sparse-checkout"""
        import subprocess
        import tempfile
        import shutil

        # Get flow info from registry
        flows = self.list_flows(use_cache=False)
        flow = next((f for f in flows if f.get('name') == name), None)

        if not flow:
            raise ValueError(f"Flow '{name}' not found in hub")

        # Get path from registry
        flow_path = flow.get('path')
        if not flow_path:
            raise ValueError(f"No path information for flow '{name}'")

        # Check if already exists
        output_path = Path(output_dir) / name
        if output_path.exists():
            raise FileExistsError(f"Flow '{name}' already exists at {output_path}")

        # Get hub URL from registry or use default
        registry = self.get_registry(use_cache=False)
        repo_url = registry.get('hub_url', 'https://github.com/mofa-org/mofa-node-hub')

        # Create temp directory for sparse checkout
        temp_dir = tempfile.mkdtemp(prefix='mofa_download_')

        try:
            # Initialize git repo
            subprocess.run(
                ['git', 'init'],
                cwd=temp_dir,
                capture_output=True,
                check=True
            )

            # Add remote
            subprocess.run(
                ['git', 'remote', 'add', 'origin', repo_url],
                cwd=temp_dir,
                capture_output=True,
                check=True
            )

            # Enable sparse checkout
            subprocess.run(
                ['git', 'config', 'core.sparseCheckout', 'true'],
                cwd=temp_dir,
                capture_output=True,
                check=True
            )

            # Specify which directory to checkout
            sparse_checkout_file = Path(temp_dir) / '.git' / 'info' / 'sparse-checkout'
            sparse_checkout_file.parent.mkdir(parents=True, exist_ok=True)
            with open(sparse_checkout_file, 'w') as f:
                f.write(f"{flow_path}\n")

            # Pull the specific directory
            subprocess.run(
                ['git', 'pull', 'origin', 'main', '--depth=1'],
                cwd=temp_dir,
                capture_output=True,
                check=True
            )

            # Copy the downloaded flow to output directory
            source = Path(temp_dir) / flow_path
            if not source.exists():
                raise RuntimeError(f"Flow directory not found after checkout: {source}")

            shutil.copytree(source, output_path)

            return True

        except subprocess.CalledProcessError as e:
            raise RuntimeError(f"Git operation failed: {e.stderr.decode() if e.stderr else str(e)}")
        except Exception as e:
            # Clean up on failure
            if output_path.exists():
                shutil.rmtree(output_path, ignore_errors=True)
            raise RuntimeError(f"Failed to download flow '{name}': {e}")
        finally:
            # Clean up temp directory
            if Path(temp_dir).exists():
                shutil.rmtree(temp_dir, ignore_errors=True)
