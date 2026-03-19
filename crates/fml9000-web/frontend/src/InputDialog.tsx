import { createSignal, createEffect, Show } from "solid-js";
import styles from "./InputDialog.module.css";

interface DialogRequest {
  title: string;
  placeholder?: string;
  defaultValue?: string;
  resolve: (value: string | null) => void;
}

const [request, setRequest] = createSignal<DialogRequest | null>(null);

export function showInputDialog(opts: {
  title: string;
  placeholder?: string;
  defaultValue?: string;
}): Promise<string | null> {
  return new Promise((resolve) => {
    setRequest({ ...opts, resolve });
  });
}

export function InputDialog() {
  let dialogRef!: HTMLDialogElement;
  let inputRef!: HTMLInputElement;

  function close(value: string | null) {
    const r = request();
    if (r) {
      r.resolve(value);
      setRequest(null);
    }
    dialogRef?.close();
  }

  function onSubmit(e: Event) {
    e.preventDefault();
    const val = inputRef.value.trim();
    close(val || null);
  }

  createEffect(() => {
    const r = request();
    if (r && dialogRef && !dialogRef.open) {
      dialogRef.showModal();
      setTimeout(() => {
        inputRef.value = r.defaultValue ?? "";
        inputRef.focus();
        inputRef.select();
      }, 0);
    }
  });

  return (
    <dialog ref={dialogRef} class={styles.dialog} onclose={() => close(null)}>
      <Show when={request()}>
        {(r) => (
          <form onsubmit={onSubmit} class={styles.form}>
            <h3 class={styles.title}>{r().title}</h3>
            <input
              ref={inputRef}
              type="text"
              class={styles.input}
              placeholder={r().placeholder ?? ""}
            />
            <div class={styles.buttons}>
              <button type="button" class={styles.cancelBtn} onclick={() => close(null)}>
                Cancel
              </button>
              <button type="submit" class={styles.submitBtn}>OK</button>
            </div>
          </form>
        )}
      </Show>
    </dialog>
  );
}
