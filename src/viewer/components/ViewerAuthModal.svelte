<script>
  import logo from '../../assets/logo.png';

  export let open = false;
  export let error = false;
  export let onSubmit = (pass) => {};

  let inputPass = '';

  function handleSubmit(e) {
    e.preventDefault();
    const trimmed = inputPass.trim();
    if (trimmed) {
      onSubmit(trimmed);
    }
  }
</script>

{#if open}
  <!-- Backdrop -->
  <div class="fixed inset-0 z-50 bg-black/85 backdrop-blur-md flex items-center justify-center p-4 animate-in fade-in duration-200 select-none">
    <!-- Dialog Card -->
    <div
      class="w-full max-w-[320px] bg-zinc-900 border border-white/10 rounded-2xl p-5 shadow-2xl flex flex-col items-center gap-3.5 text-center animate-in fade-in zoom-in-95 duration-200"
    >
      <div class="flex items-center justify-center w-12 h-12 rounded-2xl bg-zinc-950 border border-white/10 p-2.5 shadow-inner">
        <img src={logo} alt="Undetect Logo" class="w-full h-full object-contain" />
      </div>

      <div>
        <h3 class="text-sm font-bold text-white tracking-wider uppercase font-mono">Undetect Remote</h3>
        <p class="text-xs text-zinc-400 mt-0.5">Enter host PIN to unlock viewer session</p>
      </div>

      {#if error}
        <div class="w-full text-xs py-2 px-3 bg-rose-950/40 border border-rose-800/60 text-rose-300 rounded-lg">
          Incorrect PIN. Please try again.
        </div>
      {/if}

      <form onsubmit={handleSubmit} class="w-full flex flex-col gap-3 mt-0.5">
        <!-- svelte-ignore a11y_autofocus -->
        <input
          type="password"
          bind:value={inputPass}
          placeholder="••••"
          required
          autofocus
          class="bg-zinc-950 border border-zinc-800 text-white text-center text-xl tracking-widest rounded-xl focus:ring-1 focus:ring-white focus:border-white block w-full p-2.5 outline-none transition-all placeholder:text-zinc-600 font-mono"
        />
        <button
          type="submit"
          class="w-full py-2.5 px-4 text-xs font-semibold bg-white hover:bg-zinc-200 active:bg-zinc-300 text-zinc-950 rounded-xl shadow transition-all cursor-pointer active:scale-98"
        >
          Unlock Session
        </button>
      </form>
    </div>
  </div>
{/if}
