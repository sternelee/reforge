import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

interface McpStdioServer {
  command: string;
  args: string[];
  env: Record<string, string>;
}

interface McpServerConfig {
  Stdio: McpStdioServer;
}

interface McpConfig {
  mcpServers: Record<string, McpServerConfig>;
}

export function McpConfigComponent() {
  const [config, setConfig] = useState<McpConfig | null>(null);
  const [newName, setNewName] = useState('');
  const [newCommand, setNewCommand] = useState('');
  const [newArgs, setNewArgs] = useState('');

  const fetchConfig = () => {
    invoke<McpConfig>('get_mcp_config')
      .then(setConfig)
      .catch(console.error);
  };

  useEffect(() => {
    fetchConfig();
  }, []);

  const handleAddServer = () => {
    invoke('add_mcp_server', {
      name: newName,
      command: newCommand,
      args: newArgs.split(' '),
    })
      .then(() => {
        fetchConfig();
        setNewName('');
        setNewCommand('');
        setNewArgs('');
      })
      .catch(console.error);
  };

  const handleRemoveServer = (name: string) => {
    invoke('remove_mcp_server', { name })
      .then(fetchConfig)
      .catch(console.error);
  };

  if (!config) {
    return <div>Loading...</div>;
  }

  return (
    <div>
      <h2>MCP Configuration</h2>
      <div>
        <h3>Add Server</h3>
        <input
          type="text"
          placeholder="Name"
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
        />
        <input
          type="text"
          placeholder="Command"
          value={newCommand}
          onChange={(e) => setNewCommand(e.target.value)}
        />
        <input
          type="text"
          placeholder="Args (space separated)"
          value={newArgs}
          onChange={(e) => setNewArgs(e.target.value)}
        />
        <button onClick={handleAddServer}>Add</button>
      </div>
      <div>
        <h3>Servers</h3>
        <ul>
          {Object.entries(config.mcpServers).map(([name, server]) => (
            <li key={name}>
              <strong>{name}</strong>: {server.Stdio.command}{' '}
              {server.Stdio.args.join(' ')}
              <button onClick={() => handleRemoveServer(name)}>Remove</button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
