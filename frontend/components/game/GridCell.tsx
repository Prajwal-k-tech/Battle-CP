import React from 'react';
import { useDroppable } from '@dnd-kit/core';
import { cn } from '@/lib/utils';
import { motion } from 'framer-motion';

interface Props {
    id: string;
    x: number;
    y: number;
    isShip?: boolean;
    isValid?: boolean;
}

export function GridCell({ id, x, y, isShip, isValid = true }: Props) {
    const { isOver, setNodeRef } = useDroppable({
        id: id,
        data: { x, y }
    });

    return (
        <div
            ref={setNodeRef}
            className={cn(
                "w-8 h-8 sm:w-10 sm:h-10 border border-white/5 relative flex items-center justify-center transition-colors duration-200",
                isOver && "bg-white/10",
                isShip && "bg-primary/20 border-primary/50",
                isShip && !isValid && "bg-destructive/20 border-destructive/50"
            )}
        >
            <div className="w-1 h-1 bg-white/10 rounded-full" />

            {isOver && (
                <motion.div
                    layoutId="active-cell"
                    className="absolute inset-0 bg-primary/20 border border-primary box-border"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                />
            )}
        </div>
    );
}
