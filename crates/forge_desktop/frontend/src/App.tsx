import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import './App.css';

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
        <button onClick={handleListAgents}>List Agents</button>
        <ul>
          {agents.map((agent) => (
            <li key={agent}>{agent}</li>
          ))}
        </ul>
      </header>
    </div>
  );
}

export default App;
