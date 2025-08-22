import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

interface ModelId {
  id: string;
}

interface Model {
  id: ModelId;
  name?: string;
  description?: string;
  context_length?: number;
  tools_supported?: boolean;
  supports_parallel_tool_calls?: boolean;
  supports_reasoning?: boolean;
}

export function ModelSelector() {
  const [models, setModels] = useState<Model[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>('');

  useEffect(() => {
    invoke<Model[]>('list_models')
      .then((models) => {
        setModels(models);
        if (models.length > 0) {
          setSelectedModel(models[0].id.toString());
        }
      })
      .catch(console.error);
  }, []);

  const handleSetModel = (modelId: string) => {
    setSelectedModel(modelId);
    invoke('set_model', { modelId }).catch(console.error);
  };

  return (
    <div>
      <h2>Model Selection</h2>
      <select
        value={selectedModel}
        onChange={(e) => handleSetModel(e.target.value)}
      >
        {models.map((model) => (
          <option key={model.id.toString()} value={model.id.toString()}>
            {model.name || model.id.toString()}
          </option>
        ))}
      </select>
    </div>
  );
}
