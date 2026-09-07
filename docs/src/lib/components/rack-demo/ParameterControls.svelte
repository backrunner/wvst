<script lang="ts">
  import SlidersHorizontal from 'phosphor-svelte/lib/SlidersHorizontal';
  import type { DemoCopy, RackParameter } from './types';

  export let t: DemoCopy;
  export let slotId: string;
  export let parameters: RackParameter[] = [];
  export let disabled = false;
  export let onPreview: (slotId: string, parameterId: number, value: number) => void;
  export let onCommit: (slotId: string, parameterId: number, value: number) => void;

  function valueFrom(event: Event): number {
    return Number((event.currentTarget as HTMLInputElement).value);
  }
</script>

<details class="wvst-parameters" open>
  <summary><SlidersHorizontal size={17} />{t.parameters}<span>{parameters.length}</span></summary>
  {#if parameters.length === 0}
    <p class="wvst-parameter-empty">{t.noParameters}</p>
  {:else}
    <div class="wvst-parameter-grid">
      {#each parameters as parameter (parameter.info.id)}
        <label class:pending={parameter.pending}>
          <span>
            <strong>{parameter.info.shortTitle || parameter.info.title || `#${parameter.info.id}`}</strong>
            <output>{parameter.display}</output>
          </span>
          <input
            type="range"
            min="0"
            max="1"
            step={parameter.info.stepCount > 0 ? 1 / parameter.info.stepCount : .001}
            value={parameter.value}
            disabled={disabled || parameter.pending}
            on:input={(event) => onPreview(slotId, parameter.info.id, valueFrom(event))}
            on:change={(event) => onCommit(slotId, parameter.info.id, valueFrom(event))}
          />
        </label>
      {/each}
    </div>
  {/if}
</details>
