import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';

interface TaskMessage {
  text: string;
  is_md: boolean;
}

interface ChatResponse {
  TaskMessage?: TaskMessage;
  // Add other response types as needed
}

export function Chat() {
  const [message, setMessage] = useState('');
  const [history, setHistory] = useState<string[]>([]);

  useEffect(() => {
    const unlisten = listen<string>('chat_response', (event) => {
      const response: ChatResponse = JSON.parse(event.payload);
      if (response.TaskMessage) {
        setHistory((prev) => [...prev, response.TaskMessage!.text]);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleSendMessage = () => {
    setHistory((prev) => [...prev, `You: ${message}`]);
    invoke('send_chat_message', { message }).catch(console.error);
    setMessage('');
  };

  return (
    <div>
      <h2>Chat</h2>
      <div>
        {history.map((line, i) => (
          <p key={i}>{line}</p>
        ))}
      </div>
      <input
        type="text"
        value={message}
        onChange={(e) => setMessage(e.target.value)}
        onKeyPress={(e) => e.key === 'Enter' && handleSendMessage()}
      />
      <button onClick={handleSendMessage}>Send</button>
    </div>
  );
}
