import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import ReactMarkdown from 'react-markdown';

interface TaskMessage {
  text: string;
  is_md: boolean;
}

interface ToolCallFull {
  name: string;
  call_id?: string;
  arguments: any;
}

interface ToolResult {
  name: string;
  call_id?: string;
  output: any;
}

interface ChatResponse {
  TaskMessage?: TaskMessage;
  ToolCallStart?: ToolCallFull;
  ToolCallEnd?: ToolResult;
  TaskComplete?: any;
  // Add other response types as needed
}

interface Message {
  sender: 'user' | 'agent' | 'system';
  text: string;
}

export function Chat() {
  const [message, setMessage] = useState('');
  const [history, setHistory] = useState<Message[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);

  useEffect(() => {
    const unlisten = listen<string>('chat_response', (event) => {
      const response: ChatResponse = JSON.parse(event.payload);
      if (response.TaskMessage) {
        setHistory((prev) => [
          ...prev,
          { sender: 'agent', text: response.TaskMessage!.text },
        ]);
      } else if (response.ToolCallStart) {
        setHistory((prev) => [
          ...prev,
          {
            sender: 'system',
            text: `Using tool: ${response.ToolCallStart!.name}`,
          },
        ]);
      } else if (response.ToolCallEnd) {
        setHistory((prev) => [
          ...prev,
          {
            sender: 'system',
            text: `Tool ${response.ToolCallEnd!.name} finished.`,
          },
        ]);
      } else if (response.TaskComplete) {
        setIsStreaming(false);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleSendMessage = () => {
    setHistory((prev) => [...prev, { sender: 'user', text: message }]);
    setIsStreaming(true);
    invoke('send_chat_message', { message }).catch(console.error);
    setMessage('');
  };

  return (
    <div>
      <h2>Chat</h2>
      <div className="chat-history">
        {history.map((msg, i) => (
          <div key={i} className={`message ${msg.sender}`}>
            <ReactMarkdown>{msg.text}</ReactMarkdown>
          </div>
        ))}
        {isStreaming && <div className="message agent">...</div>}
      </div>
      <div className="chat-input">
        <input
          type="text"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyPress={(e) => e.key === 'Enter' && handleSendMessage()}
        />
        <button onClick={handleSendMessage}>Send</button>
      </div>
    </div>
  );
}
