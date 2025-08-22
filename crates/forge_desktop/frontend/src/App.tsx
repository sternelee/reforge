import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import './App.css';
import { McpConfigComponent } from './McpConfig';
import { ModelSelector } from './ModelSelector';
import { Chat } from './Chat';

function App() {
  const [agents, setAgents] = useState<string[]>([]);

  const handleListAgents = () => {
    invoke<string[]>('list_agents')
      .then(setAgents)
      .catch(console.error);
  };

  return (
    <div className="App">
      <header className="App-header">
        <h1>Forge Desktop</h1>
        <ModelSelector />
        <button onClick={handleListAgents}>List Agents</button>
        <ul>
          {agents.map((agent) => (
            <li key={agent}>{agent}</li>
          ))}
        </ul>
        <McpConfigComponent />
        <Chat />
      </header>
    </div>
  );
}

export default App;
