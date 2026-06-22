"""Exemplo Django — Etapa 19.

Mostra como usar o rust-py-cache como cache em memória, por processo, dentro de
uma view Django. NÃO é o backend oficial de cache do Django (isso é v0.2); é um
cache de aplicação simples, ideal para dados quentes e leitura intensiva.

Atenção: como o cache vive no processo, cada worker (gunicorn/uwsgi) tem o seu —
use para dados que podem divergir levemente entre workers, não para estado
compartilhado forte (aí use Redis).

Trecho ilustrativo (views.py de um app Django):

    from django.http import JsonResponse
    from rust_py_cache import Cache

    cache = Cache()  # idealmente em um módulo importado uma única vez

    def product_detail(request, product_id):
        key = f"product:{product_id}"
        data = cache.get(key)
        if data is None:
            product = Product.objects.get(pk=product_id)   # consulta ao banco
            data = {"id": product.id, "name": product.name, "price": str(product.price)}
            cache.set(key, data, ttl=60)
        return JsonResponse(data)

    # Decorator para memoizar funções caras (ex.: agregações/relatórios):
    @cache.cached(ttl=300)
    def monthly_report(year, month):
        ...  # cálculo pesado
        return resultado

    # Invalidação ao salvar (signal):
    from django.db.models.signals import post_save
    from django.dispatch import receiver

    @receiver(post_save, sender=Product)
    def invalidate_product_cache(sender, instance, **kwargs):
        cache.delete(f"product:{instance.id}")
"""

# O bloco abaixo roda como demonstração isolada (sem Django instalado), provando a
# mesma lógica da view acima com um "banco" fake.
from rust_py_cache import Cache

cache = Cache()

_FAKE_DB = {1: {"id": 1, "name": "Teclado", "price": "199.90"}}


def product_detail(product_id: int) -> dict:
    key = f"product:{product_id}"
    data = cache.get(key)
    if data is None:
        data = _FAKE_DB[product_id]
        cache.set(key, data, ttl=60)
    return data


if __name__ == "__main__":
    print("1ª chamada (db):  ", product_detail(1))
    print("2ª chamada (cache):", product_detail(1))
    print("stats:", cache.stats())
