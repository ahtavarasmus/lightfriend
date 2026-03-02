import { apiRequest, apiRequestMultipart } from './client';
import type { ChatRequest, ChatResponse } from './types';

export async function sendMessage(message: string): Promise<ChatResponse> {
  return apiRequest<ChatResponse>('/api/chat/web', {
    method: 'POST',
    body: JSON.stringify({ message } as ChatRequest),
  });
}

export async function sendMessageWithImage(
  message: string,
  imageUri: string,
): Promise<ChatResponse> {
  const formData = new FormData();
  formData.append('message', message);

  const filename = imageUri.split('/').pop() || 'image.jpg';
  const match = /\.(\w+)$/.exec(filename);
  const type = match ? `image/${match[1]}` : 'image/jpeg';

  formData.append('image', {
    uri: imageUri,
    name: filename,
    type,
  } as any);

  return apiRequestMultipart<ChatResponse>('/api/chat/web-with-image', formData);
}
