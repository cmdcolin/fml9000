export async function post(path: string, body?: unknown) {
  const resp = await fetch(`/api/${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  return resp.json();
}

export async function get(path: string) {
  const resp = await fetch(`/api/${path}`);
  return resp.json();
}

export async function del(path: string) {
  const resp = await fetch(`/api/${path}`, { method: "DELETE" });
  return resp.json();
}

export async function put(path: string, body: unknown) {
  const resp = await fetch(`/api/${path}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  return resp.json();
}
