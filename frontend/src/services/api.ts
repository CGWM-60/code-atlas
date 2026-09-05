export class ApiError extends Error {
  constructor(public status: number, message: string) { super(message); this.name = "ApiError"; }
}
export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(path, { ...init, headers: { "content-type": "application/json", ...init.headers } });
  const text = await response.text();
  let value: unknown;
  try { value = text ? JSON.parse(text) : undefined; }
  catch { throw new ApiError(response.status, "Le serveur a renvoyé une réponse illisible. Vérifiez qu’il est démarré, puis réessayez."); }
  if (!response.ok) throw new ApiError(response.status, typeof value === "object" && value && "error" in value ? String(value.error) : "La demande n’a pas abouti. Réessayez.");
  return value as T;
}
export const projectApi = (project: string) => `/api/projects/${encodeURIComponent(project)}`;
export const errorMessage = (error: unknown) => error instanceof Error ? error.message : "Une erreur inattendue est survenue.";
