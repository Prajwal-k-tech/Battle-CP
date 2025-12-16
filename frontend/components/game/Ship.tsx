import React from 'react';
import { useDraggable } from '@dnd-kit/core';
import { cn } from '@/lib/utils';
import { motion } from 'framer-motion';
import { RotateCw } from 'lucide-react';

interface Props {
    id: string;
    size: number;
    orientation: 'horizontal' | 'vertical';
    placed?: boolean;
    onRotate?: () => void;
}

export function Ship({ id, size, orientation, placed, onRotate }: Props) {
    const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
        id: id,
        data: { size, orientation, id }
    });

    const style = transform ? {
        transform: `translate3d(${transform.x}px, ${transform.y}px, 0)`,
    } : undefined;

    // Calculate dimensions based on generic cell size (approx 40px + gap)
    // In a real app, we might need to sync this with GridCell size via Context
    const CELL_SIZE = 40;
    const width = orientation === 'horizontal' ? size * CELL_SIZE : CELL_SIZE;
    const height = orientation === 'vertical' ? size * CELL_SIZE : CELL_SIZE;

    return (
        <div
            ref={setNodeRef}
            style={style}
            {...listeners}
            {...attributes}
            className={cn(
                "relative cursor-grab active:cursor-grabbing touch-none z-50",
                isDragging && "opacity-80 scale-105 z-[100]"
            )}
            onClick={(e) => {
                // Check if it's a quick click to rotate, but only if not dragging?
                // Dnd kit handles drag, so click might propagate.
                // Simplified: double click to rotate? or separate button.
                // User requested robust system.
            }}
        >
            <div
                className={cn(
                    "bg-primary border border-primary/50 backdrop-blur-md rounded-md flex items-center justify-center overflow-hidden transition-all",
                    "shadow-[0_0_15px_rgba(59,130,246,0.3)]"
                )}
                style={{ width: width - 4, height: height - 4, margin: 2 }} // -4 for generic gap/padding
            >
                <div className="absolute inset-0 bg-[linear-gradient(45deg,transparent_25%,rgba(255,255,255,0.1)_50%,transparent_75%)] bg-[length:10px_10px]" />

                {/* Simple visual indicator of segments */}
                <div className={cn("flex w-full h-full", orientation === 'vertical' ? "flex-col" : "flex-row")}>
                    {Array.from({ length: size }).map((_, i) => (
                        <div key={i} className="flex-1 border-white/10 border-r last:border-r-0 border-b last:border-b-0" />
                    ))}
                </div>
            </div>

            {!placed && !isDragging && (
                <button
                    className="absolute -top-6 left-1/2 -translate-x-1/2 bg-black/70 p-1.5 rounded hover:bg-primary/30 transition-colors"
                    onPointerDown={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        onRotate?.();
                    }}
                    title="Rotate ship"
                >
                    <RotateCw className="w-3 h-3 text-white" />
                </button>
            )}
        </div>
    );
}
